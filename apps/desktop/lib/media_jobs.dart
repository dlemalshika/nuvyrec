import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Optional local export tools. Capture continues to use the Rust FFmpeg library.
/// Arguments are passed directly to the executable; no shell is involved.
class MediaJobs extends ChangeNotifier {
  MediaJobs({String? toolsDirectory}) : _toolsDirectory = toolsDirectory;
  final String? _toolsDirectory;
  Process? _process;
  bool _disposed = false;
  bool busy = false, cancelled = false;
  String status = '';
  double progress = 0;
  @override
  void notifyListeners() {
    if (!_disposed) super.notifyListeners();
  }

  void notifyJobChanged() => notifyListeners();
  String get _tools =>
      _toolsDirectory ?? File(Platform.resolvedExecutable).parent.path;
  String executable(String name) => '$_tools${Platform.pathSeparator}$name.exe';
  Future<Map<String, dynamic>> probe(String path) async {
    if (!await File(path).exists()) {
      throw StateError('Recording file is missing');
    }
    final result = await Process.run(executable('ffprobe'), [
      '-v',
      'error',
      '-protocol_whitelist',
      'file,pipe',
      '-show_streams',
      '-show_format',
      '-of',
      'json',
      path,
    ]);
    if (result.exitCode != 0) throw StateError('Could not read this recording');
    return Map<String, dynamic>.from(
      jsonDecode(result.stdout as String) as Map,
    );
  }

  Future<void> export(
    String input,
    String output, {
    String mode = 'remux',
    double start = 0,
    double? duration,
    List<int>? crop,
  }) async {
    if (busy) throw StateError('Another media job is running');
    if (!await File(input).exists()) {
      throw StateError('Input recording is missing');
    }
    if (await File(output).exists()) {
      throw StateError('Choose a new output filename');
    }
    if (start < 0 ||
        !start.isFinite ||
        (duration != null && (!duration.isFinite || duration <= 0))) {
      throw ArgumentError('Invalid trim range');
    }
    if (crop != null &&
        (crop.length != 4 ||
            crop[0] < 0 ||
            crop[1] < 0 ||
            crop[2] < 2 ||
            crop[3] < 2 ||
            crop[2] % 2 != 0 ||
            crop[3] % 2 != 0)) {
      throw ArgumentError('Invalid crop coordinates');
    }
    final metadata = await probe(input);
    if (busy || _disposed) {
      throw StateError('Media tools are unavailable or busy');
    }
    final total =
        duration ??
        (double.tryParse('${(metadata['format'] as Map?)?['duration']}') ?? 0);
    final video = (metadata['streams'] as List)
        .whereType<Map>()
        .where((s) => s['codec_type'] == 'video')
        .firstOrNull;
    if (crop != null &&
        (video == null ||
            crop[0] + crop[2] > (video['width'] as int) ||
            crop[1] + crop[3] > (video['height'] as int))) {
      throw ArgumentError('Crop must fit the video');
    }
    final args = <String>[
      '-hide_banner',
      '-nostats',
      '-n',
      '-protocol_whitelist',
      'file,pipe',
    ];
    if (start > 0) args.addAll(['-ss', '$start']);
    args.addAll(['-i', input]);
    if (duration != null) args.addAll(['-t', '$duration']);
    args.addAll(['-map', '0:v:0', '-map', '0:a?']);
    if (mode == 'remux') {
      args.addAll(['-c', 'copy']);
    } else {
      if (crop != null) {
        args.addAll([
          '-vf',
          'crop=${crop[2]}:${crop[3]}:${crop[0]}:${crop[1]}',
        ]);
      }
      args.addAll([
        '-c:v',
        'libx264',
        '-preset',
        'veryfast',
        '-crf',
        '20',
        '-threads',
        '2',
        '-c:a',
        'aac',
      ]);
    }
    if (output.toLowerCase().endsWith('.mp4')) {
      args.addAll(['-movflags', '+faststart']);
    }
    args.addAll(['-progress', 'pipe:1', output]);
    busy = true;
    cancelled = false;
    progress = 0;
    status = 'Exporting…';
    notifyListeners();
    String errors = '';
    try {
      await const MethodChannel(
        'org.screenrecorder/host',
      ).invokeMethod<void>('setMediaBusy', true);
      final process = await Process.start(
        executable('ffmpeg'),
        args,
        runInShell: false,
      );
      _process = process;
      if (cancelled) cancel();
      final stderr = process.stderr.transform(utf8.decoder).listen((text) {
        errors += text;
        if (errors.length > 4000) {
          errors = errors.substring(errors.length - 4000);
        }
      });
      final stdout = process.stdout
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen((line) {
            if (line.startsWith('out_time_us=') && total > 0) {
              progress =
                  ((double.tryParse(line.substring(12)) ?? 0) / 1000000 / total)
                      .clamp(0, 1);
              notifyListeners();
            }
          });
      final code = await process.exitCode;
      await stderr.cancel();
      await stdout.cancel();
      if (cancelled) {
        status = 'Export cancelled; any partial output is kept for inspection';
      } else if (code != 0) {
        throw StateError(errors.isEmpty ? 'Export failed' : errors);
      } else {
        status = 'Export saved';
        progress = 1;
      }
    } finally {
      try {
        await const MethodChannel(
          'org.screenrecorder/host',
        ).invokeMethod<void>('setMediaBusy', false);
      } catch (_) {}
      _process = null;
      busy = false;
      notifyListeners();
    }
  }

  void cancel() {
    cancelled = true;
    try {
      _process?.stdin.writeln('q');
    } catch (_) {}
    final process = _process;
    Timer(const Duration(seconds: 2), () {
      if (identical(_process, process)) process?.kill();
    });
  }

  @override
  void dispose() {
    _disposed = true;
    if (busy) cancel();
    super.dispose();
  }
}
