import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'package:flutter/foundation.dart';

/// SemVer precedence ignores build metadata; prereleases sort below the final
/// release, with numeric identifiers below textual identifiers (SemVer 2.0).
class AppVersion implements Comparable<AppVersion> {
  AppVersion._(this.core, this.pre);
  final List<BigInt> core;
  final List<String> pre;
  static AppVersion? parse(String text) {
    if (text.length > 128) return null;
    final match = RegExp(
      r'^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z.-]+))?(?:\+([0-9A-Za-z.-]+))?$',
    ).firstMatch(text);
    if (match == null) return null;
    final pre = match[4]?.split('.') ?? <String>[];
    final build = match[5]?.split('.') ?? <String>[];
    if ([...pre, ...build].any((s) => s.isEmpty) ||
        pre.any(
          (s) =>
              RegExp(r'^\d+$').hasMatch(s) && s.length > 1 && s.startsWith('0'),
        )) {
      return null;
    }
    return AppVersion._([
      for (var i = 1; i <= 3; i++) BigInt.parse(match[i]!),
    ], pre);
  }

  @override
  int compareTo(AppVersion other) {
    for (var i = 0; i < 3; i++) {
      final order = core[i].compareTo(other.core[i]);
      if (order != 0) return order;
    }
    if (pre.isEmpty || other.pre.isEmpty) {
      return pre.isEmpty == other.pre.isEmpty
          ? 0
          : pre.isEmpty
          ? 1
          : -1;
    }
    for (var i = 0; i < pre.length && i < other.pre.length; i++) {
      final numeric = RegExp(r'^\d+$');
      final a = numeric.hasMatch(pre[i]) ? BigInt.parse(pre[i]) : null;
      final b = numeric.hasMatch(other.pre[i])
          ? BigInt.parse(other.pre[i])
          : null;
      final order = a != null && b != null
          ? a.compareTo(b)
          : a != null
          ? -1
          : b != null
          ? 1
          : pre[i].compareTo(other.pre[i]);
      if (order != 0) return order;
    }
    return pre.length.compareTo(other.pre.length);
  }
}

String normalizeRepository(String input) {
  var value = input.trim();
  if (value.startsWith('https://')) {
    final uri = Uri.tryParse(value);
    if (uri == null ||
        uri.host != 'github.com' ||
        uri.userInfo.isNotEmpty ||
        uri.hasQuery ||
        uri.hasFragment ||
        (uri.hasPort && uri.port != 443)) {
      throw const FormatException(
        'Use a public github.com repository URL or owner/repository.',
      );
    }
    value = uri.path
        .replaceFirst(RegExp(r'^/'), '')
        .replaceFirst(RegExp(r'/$'), '');
  }
  value = value.replaceFirst(RegExp(r'\.git$'), '');
  if (!RegExp(
        r'^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})/[A-Za-z0-9_.-]{1,100}$',
      ).hasMatch(value) ||
      ['.', '..'].contains(value.split('/').last)) {
    throw const FormatException(
      'Use owner/repository, for example dlemalshika/screenrecorder.',
    );
  }
  return value;
}

class AppRelease {
  AppRelease(this.tag, this.notes, this.repository, this.hasWindowsBundle);
  final String tag, notes, repository;
  final bool hasWindowsBundle;
  Uri get url => Uri.https('github.com', '/$repository/releases/tag/$tag');
  Map<String, dynamic> toJson() => {
    'tag_name': tag,
    'body': notes,
    'draft': false,
    'prerelease': AppVersion.parse(tag)!.pre.isNotEmpty,
    'has_windows_bundle': hasWindowsBundle,
  };
  static AppRelease? fromJson(Map data, String repository, bool beta) {
    final tag = data['tag_name'];
    if (tag is! String || data['draft'] != false) return null;
    final version = AppVersion.parse(tag);
    if (version == null ||
        (!beta && (data['prerelease'] != false || version.pre.isNotEmpty))) {
      return null;
    }
    final body = data['body'];
    final assets = data['assets'];
    final bundle =
        data['has_windows_bundle'] == true ||
        (assets is List &&
            assets.whereType<Map>().any((a) {
              final name = a['name'];
              return name is String &&
                  RegExp(
                    r'^Frame-Windows[^/\\]*\.(zip|exe|msi)$',
                    caseSensitive: false,
                  ).hasMatch(name) &&
                  a['state'] == 'uploaded';
            }));
    return AppRelease(
      tag,
      body is String ? body.substring(0, body.length.clamp(0, 12000)) : '',
      repository,
      bundle,
    );
  }
}

typedef ReleaseFetcher =
    Future<AppRelease?> Function(String repository, bool beta);

/// Public GitHub metadata only. No token, user telemetry, capture bytes or
/// stream credentials are sent. Redirects are disabled and response/time budgets
/// bound malformed or stalled servers. Beta examines at most 100 recent releases.
Future<AppRelease?> fetchGitHubRelease(String repository, bool beta) async {
  repository = normalizeRepository(repository);
  final client = HttpClient()..connectionTimeout = const Duration(seconds: 8);
  try {
    return await (() async {
      final uri = Uri.https(
        'api.github.com',
        '/repos/$repository/releases${beta ? '' : '/latest'}',
        beta ? {'per_page': '100'} : null,
      );
      final request = await client.getUrl(uri);
      request.followRedirects = false;
      request.headers.set(HttpHeaders.userAgentHeader, 'Frame-Update-Checker');
      request.headers.set(
        HttpHeaders.acceptHeader,
        'application/vnd.github+json',
      );
      request.headers.set('X-GitHub-Api-Version', '2026-03-10');
      final response = await request.close();
      if (response.statusCode == 404) return null;
      if (response.statusCode == 403 || response.statusCode == 429) {
        throw const HttpException(
          'GitHub request limit reached. Try again later.',
        );
      }
      if (response.statusCode != 200) {
        throw HttpException('GitHub returned HTTP ${response.statusCode}.');
      }
      final bytes = BytesBuilder(copy: false);
      await for (final chunk in response) {
        if (bytes.length + chunk.length > 4 * 1024 * 1024) {
          throw const HttpException(
            'Release metadata exceeds the supported size.',
          );
        }
        bytes.add(chunk);
      }
      final decoded = jsonDecode(utf8.decode(bytes.takeBytes()));
      final values = beta
          ? decoded is List
                ? decoded
                : throw const FormatException('Invalid release list')
          : decoded is Map
          ? [decoded]
          : throw const FormatException('Invalid release metadata');
      AppRelease? best;
      for (final value in values.whereType<Map>().take(100)) {
        final release = AppRelease.fromJson(value, repository, beta);
        if (release != null &&
            (best == null ||
                AppVersion.parse(
                      release.tag,
                    )!.compareTo(AppVersion.parse(best.tag)!) >
                    0)) {
          best = release;
        }
      }
      return best;
    })().timeout(const Duration(seconds: 15));
  } finally {
    client.close(force: true);
  }
}

class UpdateController extends ChangeNotifier {
  UpdateController({ReleaseFetcher? fetcher, DateTime Function()? clock})
    : _fetcher = fetcher ?? fetchGitHubRelease,
      _clock = clock ?? DateTime.now;
  final ReleaseFetcher _fetcher;
  final DateTime Function() _clock;
  String repository = const String.fromEnvironment(
    'FRAME_UPDATE_REPOSITORY',
    defaultValue: 'dlemalshika/screenrecorder',
  );
  String? currentVersion, error, message, skipped;
  bool automatic = true, beta = false, checking = false, _disposed = false;
  DateTime? lastCheck, deferredUntil;
  AppRelease? available;
  Timer? _timer, _startup;
  VoidCallback? onPersist;
  bool get notifyUpdate =>
      available != null &&
      available!.tag != skipped &&
      (deferredUntil == null || !_clock().isBefore(deferredUntil!));
  bool get due =>
      lastCheck == null ||
      _clock().difference(lastCheck!) >= const Duration(days: 1);

  void restore(Map data) {
    try {
      if (data['repository'] is String) {
        repository = normalizeRepository(data['repository']);
      }
    } catch (_) {}
    automatic = data['automatic'] != false;
    beta = data['beta'] == true;
    skipped = data['skipped'] is String ? data['skipped'] : null;
    lastCheck = DateTime.tryParse(data['last_check']?.toString() ?? '');
    deferredUntil = DateTime.tryParse(data['deferred_until']?.toString() ?? '');
    final saved = data['release'];
    final current = AppVersion.parse(currentVersion ?? '');
    if (saved is Map && current != null) {
      final release = AppRelease.fromJson(saved, repository, beta);
      if (release != null &&
          AppVersion.parse(release.tag)!.compareTo(current) > 0) {
        available = release;
      }
    }
  }

  Map<String, dynamic> toJson() => {
    'repository': repository,
    'automatic': automatic,
    'beta': beta,
    'skipped': skipped,
    'last_check': lastCheck?.toUtc().toIso8601String(),
    'deferred_until': deferredUntil?.toUtc().toIso8601String(),
    'release': available?.toJson(),
  };

  void start(bool Function() idle) {
    _startup = Timer(const Duration(seconds: 15), () {
      if (idle()) check();
    });
    _timer = Timer.periodic(const Duration(hours: 1), (_) {
      if (idle()) check();
    });
  }

  void configure(String repo, bool includeBeta, bool auto) {
    final next = normalizeRepository(repo);
    if (next != repository || includeBeta != beta) {
      available = null;
      skipped = null;
      deferredUntil = null;
      lastCheck = null;
    }
    repository = next;
    beta = includeBeta;
    automatic = auto;
    error = null;
    onPersist?.call();
    notifyListeners();
  }

  void later() {
    deferredUntil = _clock().add(const Duration(days: 1));
    onPersist?.call();
    notifyListeners();
  }

  void skip() {
    skipped = available?.tag;
    onPersist?.call();
    notifyListeners();
  }

  Future<void> check({bool manual = false}) async {
    if (checking ||
        _disposed ||
        (!manual && (!automatic || !due || currentVersion == null))) {
      return;
    }
    final current = AppVersion.parse(currentVersion ?? '');
    if (current == null) {
      error = 'Installed version is unavailable; restart the Windows app.';
      notifyListeners();
      return;
    }
    // Capture configuration: changing repositories while a request is in flight
    // must never attach the previous repository's release to the new source.
    final repo = repository, channel = beta;
    checking = true;
    error = null;
    message = null;
    notifyListeners();
    try {
      final release = await _fetcher(repo, channel);
      if (_disposed || repo != repository || channel != beta) return;
      lastCheck = _clock();
      available =
          release != null &&
              AppVersion.parse(release.tag)!.compareTo(current) > 0
          ? release
          : null;
      if (manual) {
        message = release == null
            ? 'No published release found. The repository may be private or have no releases yet.'
            : available == null
            ? 'You are up to date.'
            : 'Update ${release.tag} is available.';
      }
      if (manual) deferredUntil = null;
      onPersist?.call();
    } catch (_) {
      if (!_disposed && repo == repository && channel == beta) {
        lastCheck =
            _clock(); // no background retry storm when offline/rate limited
        error =
            'Could not check GitHub. Check your connection and public repository, then try again later.';
        onPersist?.call();
      }
    } finally {
      if (!_disposed) {
        checking = false;
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _timer?.cancel();
    _startup?.cancel();
    super.dispose();
  }
}
