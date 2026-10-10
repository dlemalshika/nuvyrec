import 'dart:ffi';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'package:ffi/ffi.dart';

/// Bootstrap binding. The host supplies an already bundled native library.
///
/// Windows/macOS: load the packaged DLL/dylib by an application-controlled path.
/// iOS: use DynamicLibrary.process() after statically linking the Rust library
/// and explicitly retaining recorder_abi_version from linker dead stripping.
/// Android: load the ABI-specific bundled librecorder_core.so.
///
/// Recording commands will be asynchronous and owned by a native worker; do not
/// put encoding calls on the Flutter UI isolate or send raw frames through Dart.
final class RecorderBridge {
  RecorderBridge(DynamicLibrary library)
    : _abiVersion = library.lookupFunction<Uint32 Function(), int Function()>(
        'recorder_abi_version',
      ) {
    if (abiVersion >> 16 != 1) {
      throw StateError('Unsupported recorder core ABI version: $abiVersion');
    }
  }

  final int Function() _abiVersion;

  int get abiVersion => _abiVersion();
}

/// Owned-string ABI binding. Input allocations use Dart's allocator; output
/// allocations are released exclusively through the Rust destructor.
final class NativeRecorder {
  NativeRecorder(DynamicLibrary library)
    : _sources = library
          .lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>(
            'recorder_sources_json',
          ),
      _status = library
          .lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>(
            'recorder_status_json',
          ),
      _cameras = library
          .lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>(
            'recorder_cameras_json',
          ),
      _cameraRequest = library
          .lookupFunction<
            Pointer<Utf8> Function(Pointer<Utf8>),
            Pointer<Utf8> Function(Pointer<Utf8>)
          >('recorder_webcam_preview_request_json'),
      _cameraFrame = library
          .lookupFunction<
            Pointer<Uint8> Function(
              Pointer<UintPtr>,
              Pointer<Uint32>,
              Pointer<Uint32>,
            ),
            Pointer<Uint8> Function(
              Pointer<UintPtr>,
              Pointer<Uint32>,
              Pointer<Uint32>,
            )
          >('recorder_webcam_preview_rgba'),
      _previewFree = library
          .lookupFunction<
            Void Function(Pointer<Uint8>, UintPtr),
            void Function(Pointer<Uint8>, int)
          >('recorder_preview_free'),
      _command = library
          .lookupFunction<
            Pointer<Utf8> Function(Pointer<Utf8>),
            Pointer<Utf8> Function(Pointer<Utf8>)
          >('recorder_command_json'),
      _layer = library
          .lookupFunction<
            Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Uint8>, UintPtr),
            Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Uint8>, int)
          >('recorder_layer_rgba_json'),
      _release = library
          .lookupFunction<
            Void Function(Pointer<Utf8>),
            void Function(Pointer<Utf8>)
          >('recorder_string_free') {
    RecorderBridge(library);
  }

  factory NativeRecorder.open() {
    final directory = File(Platform.resolvedExecutable).parent.path;
    final path = Platform.isWindows
        ? '$directory\\recorder_core.dll'
        : '$directory/librecorder_core.dylib';
    return NativeRecorder(DynamicLibrary.open(path));
  }

  final Pointer<Utf8> Function() _sources, _status, _cameras;
  final Pointer<Utf8> Function(Pointer<Utf8>) _command;
  final Pointer<Utf8> Function(Pointer<Utf8>) _cameraRequest;
  final Pointer<Uint8> Function(
    Pointer<UintPtr>,
    Pointer<Uint32>,
    Pointer<Uint32>,
  )
  _cameraFrame;
  final void Function(Pointer<Uint8>, int) _previewFree;
  final Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Uint8>, int) _layer;
  final void Function(Pointer<Utf8>) _release;

  Map<String, dynamic> _read(Pointer<Utf8> pointer) {
    if (pointer == nullptr)
      throw StateError('Native recorder could not allocate a response.');
    try {
      return jsonDecode(pointer.toDartString()) as Map<String, dynamic>;
    } finally {
      _release(pointer);
    }
  }

  Map<String, dynamic> sources() => _read(_sources());
  Map<String, dynamic> status() => _read(_status());
  Map<String, dynamic> cameras() => _read(_cameras());
  void requestWebcamPreview(Map<String, dynamic> webcam, bool lowResource) {
    final text = jsonEncode({
      'webcam': webcam,
      'low_resource': lowResource,
    }).toNativeUtf8();
    try {
      final result = _read(_cameraRequest(text));
      if (result['ok'] != true || result['error'] is String) {
        throw StateError(
          result['error']?.toString() ?? 'Webcam preview failed',
        );
      }
    } finally {
      calloc.free(text);
    }
  }

  WebcamPreviewFrame? webcamPreviewFrame() {
    final length = calloc<UintPtr>();
    final width = calloc<Uint32>();
    final height = calloc<Uint32>();
    Pointer<Uint8> pixels = nullptr;
    try {
      pixels = _cameraFrame(length, width, height);
      if (pixels == nullptr) return null;
      if (width.value < 1 ||
          width.value > 320 ||
          height.value < 1 ||
          height.value > 240 ||
          length.value != width.value * height.value * 4) {
        throw StateError('Invalid native camera thumbnail');
      }
      return WebcamPreviewFrame(
        width.value,
        height.value,
        Uint8List.fromList(pixels.asTypedList(length.value)),
      );
    } finally {
      if (pixels != nullptr) _previewFree(pixels, length.value);
      calloc.free(length);
      calloc.free(width);
      calloc.free(height);
    }
  }

  /// Uploads a small overlay asset, never captured screen/video pixels.
  void uploadLayer(Map<String, dynamic> metadata, Uint8List rgba) {
    if (rgba.isEmpty || rgba.length > 2 * 1024 * 1024)
      throw ArgumentError('Invalid overlay buffer');
    final text = jsonEncode(metadata).toNativeUtf8();
    final pixels = calloc<Uint8>(rgba.length);
    try {
      pixels.asTypedList(rgba.length).setAll(0, rgba);
      final result = _read(_layer(text, pixels, rgba.length));
      if (result['ok'] != true)
        throw StateError(
          result['error']?.toString() ?? 'Overlay upload failed',
        );
    } finally {
      calloc.free(pixels);
      calloc.free(text);
    }
  }

  void command(Map<String, dynamic> command) {
    final input = jsonEncode(command).toNativeUtf8();
    try {
      final result = _read(_command(input));
      if (result['ok'] != true)
        throw StateError(
          result['error']?.toString() ?? 'Native command failed.',
        );
    } finally {
      calloc.free(input);
    }
  }
}

/// Small latest-only UI thumbnail, not a recording frame queue.
final class WebcamPreviewFrame {
  WebcamPreviewFrame(this.width, this.height, this.rgba);
  final int width, height;
  final Uint8List rgba;
}
