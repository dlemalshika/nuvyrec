import 'dart:ffi';
import 'dart:convert';
import 'dart:io';
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
      _command = library
          .lookupFunction<
            Pointer<Utf8> Function(Pointer<Utf8>),
            Pointer<Utf8> Function(Pointer<Utf8>)
          >('recorder_command_json'),
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

  final Pointer<Utf8> Function() _sources, _status;
  final Pointer<Utf8> Function(Pointer<Utf8>) _command;
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
