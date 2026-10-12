import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_desktop/app_updates.dart';
import 'package:recorder_desktop/update_panel.dart';
import 'package:recorder_desktop/controller.dart';

AppRelease release(String tag, {bool beta = false, bool draft = false}) =>
    AppRelease.fromJson(
      {
        'tag_name': tag,
        'draft': draft,
        'prerelease': beta,
        'body': 'New features',
        'assets': [
          {'name': 'Nuvyrec-Windows-x64.zip', 'state': 'uploaded'},
        ],
      },
      'dlemalshika/nuvyrec',
      true,
    )!;

void main() {
  test('retired repository migrates without retaining stale release state', () {
    final updates = UpdateController()..currentVersion = '0.3.1';
    expect(updates.repository, 'dlemalshika/nuvyrec');
    updates.restore({
      'repository': 'dlemalshika/screenrecorder',
      'automatic': false,
      'skipped': 'v0.4.0',
      'last_check': '2026-10-10T00:00:00Z',
      'release': release('v0.4.0').toJson(),
    });
    expect(updates.repository, 'dlemalshika/nuvyrec');
    expect(updates.automatic, false);
    expect(updates.available, isNull);
    expect(updates.lastCheck, isNull);
    expect(updates.skipped, isNull);
    updates.restore({'repository': 'another/project'});
    expect(updates.repository, 'another/project');
    updates.dispose();
  });

  test(
    'SemVer precedence covers numeric tags, prereleases, textual minus and build metadata',
    () {
      final versions = [
        '1.0.0-alpha',
        '1.0.0-alpha.1',
        '1.0.0-alpha.beta',
        '1.0.0-beta',
        '1.0.0-beta.2',
        '1.0.0-beta.11',
        '1.0.0-rc.1',
        '1.0.0',
        '1.9.0',
        '1.10.0',
      ];
      for (var i = 1; i < versions.length; i++) {
        expect(
          AppVersion.parse(
            versions[i],
          )!.compareTo(AppVersion.parse(versions[i - 1])!),
          greaterThan(0),
        );
      }
      expect(
        AppVersion.parse('v1.0.0+10')!.compareTo(AppVersion.parse('1.0.0+2')!),
        0,
      );
      expect(
        AppVersion.parse('1.0.0--1')!.compareTo(AppVersion.parse('1.0.0-1')!),
        greaterThan(0),
      );
      expect(
        AppVersion.parse(
          '1.0.0-0x20',
        )!.compareTo(AppVersion.parse('1.0.0-99')!),
        greaterThan(0),
      );
      for (final invalid in [
        '1.0',
        '01.0.0',
        '1.0.0-beta.01',
        '1.0.0-beta..1',
        '1.0.0+',
        'file://1.0.0',
      ]) {
        expect(AppVersion.parse(invalid), isNull);
      }
    },
  );
  test(
    'repository validation rejects credentials, arbitrary hosts and path injection',
    () {
      expect(
        normalizeRepository('https://github.com/dlemalshika/nuvyrec.git'),
        'dlemalshika/nuvyrec',
      );
      for (final invalid in [
        'https://evil.test/a/b',
        'https://user:secret@github.com/a/b',
        'https://github.com/a/b?token=secret',
        'https://github.com/a/b#fragment',
        'a/../b',
        'a/..',
        'file:///a/b',
        'a/b/releases',
      ]) {
        expect(() => normalizeRepository(invalid), throwsFormatException);
      }
    },
  );
  test('stable excludes draft/beta and constructs only a release page URL', () {
    final data = {
      'tag_name': 'v0.4.0-beta.1',
      'draft': false,
      'prerelease': true,
      'body': 'Notes',
      'html_url': 'file:///bad.exe',
    };
    expect(AppRelease.fromJson(data, 'a/b', false), isNull);
    expect(AppRelease.fromJson({...data, 'draft': true}, 'a/b', true), isNull);
    expect(
      AppRelease.fromJson(data, 'a/b', true)!.url.toString(),
      'https://github.com/a/b/releases/tag/v0.4.0-beta.1',
    );
  });
  test(
    'daily checks, Later and Skip persist, manual checks ignore the daily interval',
    () async {
      var now = DateTime.utc(2026, 10, 11), requests = 0;
      final updates = UpdateController(
        clock: () => now,
        fetcher: (repo, beta) async {
          requests++;
          return release('v0.4.0');
        },
      )..currentVersion = '0.3.0+3';
      await updates.check();
      expect(updates.notifyUpdate, true);
      await updates.check();
      expect(requests, 1);
      updates.later();
      expect(updates.notifyUpdate, false);
      now = now.add(const Duration(days: 1));
      expect(updates.notifyUpdate, true);
      updates.skip();
      expect(updates.notifyUpdate, false);
      final restored = UpdateController(clock: () => now)
        ..currentVersion = '0.3.0';
      restored.restore(updates.toJson());
      expect(restored.available!.tag, 'v0.4.0');
      expect(restored.notifyUpdate, false);
      await updates.check(manual: true);
      expect(requests, 2);
      updates.configure('another/project', false, false);
      expect(updates.available, isNull);
      expect(updates.skipped, isNull);
      await updates.check();
      expect(requests, 2);
      updates.dispose();
      restored.dispose();
    },
  );
  test(
    'no downgrade/build-only update, offline backoff and no-release state',
    () async {
      final updates = UpdateController(
        fetcher: (_, _) async => release('v0.3.0+100'),
      )..currentVersion = '0.3.0+3';
      await updates.check(manual: true);
      expect(updates.available, isNull);
      final offline = UpdateController(
        fetcher: (_, _) async => throw Exception('offline'),
      )..currentVersion = '0.3.0';
      await offline.check();
      expect(offline.error, isNotNull);
      expect(offline.due, false);
      final empty = UpdateController(fetcher: (_, _) async => null)
        ..currentVersion = '0.3.0';
      await empty.check(manual: true);
      expect(empty.message, contains('No published release'));
      updates.dispose();
      offline.dispose();
      empty.dispose();
    },
  );
  test(
    'single-flight checker discards responses from a changed repository and disposal',
    () async {
      final result = Completer<AppRelease?>();
      var requests = 0;
      final updates = UpdateController(
        fetcher: (_, _) {
          requests++;
          return result.future;
        },
      )..currentVersion = '0.3.0';
      final pending = updates.check(manual: true);
      await updates.check(manual: true);
      expect(requests, 1);
      updates.configure('new/project', false, true);
      result.complete(release('v0.4.0'));
      await pending;
      expect(updates.available, isNull);
      expect(updates.lastCheck, isNull);
      updates.dispose();
      final delayed = Completer<AppRelease?>();
      final disposed = UpdateController(fetcher: (_, _) => delayed.future)
        ..currentVersion = '0.3.0';
      final task = disposed.check();
      disposed.dispose();
      delayed.complete(release('v0.4.0'));
      await task;
      expect(disposed.available, isNull);
    },
  );
  testWidgets(
    'background timer defers requests while recording, then checks when idle',
    (tester) async {
      var idle = false, requests = 0;
      final updates = UpdateController(
        fetcher: (_, _) async {
          requests++;
          return release('v0.4.0');
        },
      )..currentVersion = '0.3.0';
      updates.start(() => idle);
      await tester.pump(const Duration(seconds: 16));
      expect(requests, 0);
      idle = true;
      await tester.pump(const Duration(hours: 1));
      expect(requests, 1);
      updates.dispose();
    },
  );
  for (final brightness in Brightness.values) {
    testWidgets(
      'update panel fits compact ${brightness.name} and opens only GitHub in browser',
      (tester) async {
        tester.view.physicalSize = const Size(960, 640);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        final updates = UpdateController(
          fetcher: (_, _) async => release('v0.4.0'),
        )..currentVersion = '0.3.0';
        String? opened;
        tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          RecorderController.host,
          (call) async {
            expect(call.method, 'openReleasePage');
            opened = (call.arguments as Map)['url'] as String;
            return null;
          },
        );
        await updates.check(manual: true);
        await tester.pumpWidget(
          MaterialApp(
            theme: ThemeData(brightness: brightness),
            home: Scaffold(
              body: Center(
                child: SizedBox(
                  width: 480,
                  child: SingleChildScrollView(
                    child: UpdatePanel(updates: updates),
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        await tester.ensureVisible(find.text('Download on GitHub'));
        await tester.tap(find.text('Download on GitHub'));
        await tester.pumpAndSettle();
        expect(
          opened,
          'https://github.com/dlemalshika/nuvyrec/releases/tag/v0.4.0',
        );
        await tester.pumpWidget(const SizedBox());
        updates.dispose();
        tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          RecorderController.host,
          null,
        );
      },
    );
  }
}
