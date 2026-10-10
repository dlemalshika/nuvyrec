import 'package:flutter/widgets.dart';
import 'recorder_app.dart';
import 'creator_smoke.dart';

void main(List<String> args) {
  final fixture = args.indexOf('--creator-smoke-test');
  if (fixture >= 0 && fixture + 1 < args.length) {
    runApp(CreatorSmoke(directory: args[fixture + 1]));
  } else {
    runApp(const RecorderApp());
  }
}
