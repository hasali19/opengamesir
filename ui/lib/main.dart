import 'dart:async';

import 'package:dbus/dbus.dart';
import 'package:flutter/material.dart';
import 'package:gap/gap.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'OpenGameSir',
      theme: ThemeData.light(),
      darkTheme: ThemeData.dark(),
      home: const MyHomePage(title: 'Flutter Demo Home Page'),
    );
  }
}

class Device {
  final DBusObjectPath path;
  final String syspath;
  final int vendorId;
  final int productId;
  final String friendlyName;

  Device({
    required this.path,
    required this.syspath,
    required this.vendorId,
    required this.productId,
    required this.friendlyName,
  });
}

class MyHomePage extends StatefulWidget {
  const MyHomePage({super.key, required this.title});

  final String title;

  @override
  State<MyHomePage> createState() => _MyHomePageState();
}

class _MyHomePageState extends State<MyHomePage> {
  List<Device> devices = [];

  late DBusClient _client;
  late StreamSubscription<DBusSignal> _deviceAddedSub;
  late StreamSubscription<DBusSignal> _deviceRemovedSub;

  @override
  void initState() {
    super.initState();

    _client = DBusClient.session();

    final devicesObj = DBusRemoteObject(
      _client,
      name: 'dev.hasali.OpenGameSir',
      path: DBusObjectPath('/dev/hasali/OpenGameSir/Devices'),
    );

    final deviceAddedSignal = DBusRemoteObjectSignalStream(
      object: devicesObj,
      interface: 'dev.hasali.OpenGameSir.Devices',
      name: 'DeviceAdded',
    );

    final deviceRemovedSignal = DBusRemoteObjectSignalStream(
      object: devicesObj,
      interface: 'dev.hasali.OpenGameSir.Devices',
      name: 'DeviceRemoved',
    );

    _deviceAddedSub = deviceAddedSignal.listen((signal) {
      final deviceStruct = signal.values.first.asStruct();
      setState(() {
        devices.add(
          Device(
            path: deviceStruct[0].asObjectPath(),
            syspath: deviceStruct[1].asString(),
            vendorId: deviceStruct[2].asUint16(),
            productId: deviceStruct[3].asUint16(),
            friendlyName: deviceStruct[4].asString(),
          ),
        );
      });
    });

    _deviceRemovedSub = deviceRemovedSignal.listen((signal) {
      final deviceStruct = signal.values.first.asStruct();
      final path = deviceStruct[0].asObjectPath();
      setState(() {
        devices.removeWhere((device) => device.path == path);
      });
    });

    Future.microtask(() async {
      final ret = await devicesObj.callMethod(
        'dev.hasali.OpenGameSir.Devices',
        'GetDevices',
        [],
      );

      final fetched = ret.returnValues.first.asArray();
      final loaded = fetched.map((device) {
        final deviceStruct = device.asStruct();
        return Device(
          path: deviceStruct[0].asObjectPath(),
          syspath: deviceStruct[1].asString(),
          vendorId: deviceStruct[2].asUint16(),
          productId: deviceStruct[3].asUint16(),
          friendlyName: deviceStruct[4].asString(),
        );
      }).toList();

      if (mounted) {
        setState(() {
          devices.addAll(loaded);
        });
      }
    });
  }

  @override
  void dispose() {
    _deviceAddedSub.cancel();
    _deviceRemovedSub.cancel();
    _client.close();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    var textTheme = Theme.of(context).textTheme;
    return Scaffold(
      body: Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (devices.isEmpty)
              Text('No device connected', style: textTheme.titleLarge)
            else ...[
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text('Connected', style: textTheme.titleLarge),
                  Gap(8),
                  Icon(Icons.check_circle_outline),
                ],
              ),
              Text(devices.first.friendlyName, style: textTheme.titleMedium),
            ],
          ],
        ),
      ),
    );
  }
}
