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
  final String syspath;
  final int vendorId;
  final int productId;
  final String friendlyName;
  final DBusRemoteObject object;

  Device({
    required this.syspath,
    required this.vendorId,
    required this.productId,
    required this.friendlyName,
    required this.object,
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
  int? batteryLevel;

  late DBusClient _client;
  late StreamSubscription<DBusSignal> _deviceAddedSub;
  late StreamSubscription<DBusSignal> _deviceRemovedSub;
  late ConnectionStatusManager _connectionStatusManager;

  Timer? _statusPoller;

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

    _deviceAddedSub = deviceAddedSignal.listen((signal) async {
      final deviceStruct = signal.values.first.asStruct();

      setState(() {
        devices.add(
          Device(
            syspath: deviceStruct[1].asString(),
            vendorId: deviceStruct[2].asUint16(),
            productId: deviceStruct[3].asUint16(),
            friendlyName: deviceStruct[4].asString(),
            object: DBusRemoteObject(
              _client,
              name: 'dev.hasali.OpenGameSir',
              path: deviceStruct[0].asObjectPath(),
            ),
          ),
        );
      });

      if (devices.firstOrNull case Device device) {
        final result = await device.object.getProperty(
          'dev.hasali.OpenGameSir.Device',
          'BatteryLevel',
        );
        final batteryLevel = result.asByte();
        setState(() {
          this.batteryLevel = batteryLevel;
        });
      }
    });

    _deviceRemovedSub = deviceRemovedSignal.listen((signal) {
      final deviceStruct = signal.values.first.asStruct();
      final path = deviceStruct[0].asObjectPath();
      setState(() {
        devices.removeWhere((device) => device.object.path == path);
      });
    });

    _connectionStatusManager = ConnectionStatusManager(_client);

    _connectionStatusManager.addListener(() async {
      if (_connectionStatusManager.isConnected) {
        final ret = await devicesObj.callMethod(
          'dev.hasali.OpenGameSir.Devices',
          'GetDevices',
          [],
        );

        final fetched = ret.returnValues.first.asArray();
        final loaded = fetched.map((device) {
          final deviceStruct = device.asStruct();
          return Device(
            syspath: deviceStruct[1].asString(),
            vendorId: deviceStruct[2].asUint16(),
            productId: deviceStruct[3].asUint16(),
            friendlyName: deviceStruct[4].asString(),
            object: DBusRemoteObject(
              _client,
              name: 'dev.hasali.OpenGameSir',
              path: deviceStruct[0].asObjectPath(),
            ),
          );
        }).toList();

        _statusPoller = Timer.periodic(Duration(seconds: 30), (timer) async {
          if (devices.firstOrNull case Device device) {
            final result = await device.object.getProperty(
              'dev.hasali.OpenGameSir.Device',
              'BatteryLevel',
            );
            final batteryLevel = result.asByte();
            setState(() {
              this.batteryLevel = batteryLevel;
            });
          }
        });

        if (mounted) {
          setState(() {
            devices.addAll(loaded);
          });
        }

        if (devices.firstOrNull case Device device) {
          final result = await device.object.getProperty(
            'dev.hasali.OpenGameSir.Device',
            'BatteryLevel',
          );
          final batteryLevel = result.asByte();
          setState(() {
            this.batteryLevel = batteryLevel;
          });
        }
      } else {
        _statusPoller?.cancel();
        devices.clear();
        batteryLevel = null;
      }

      setState(() {});
    });

    Future.microtask(_connectionStatusManager.init);
  }

  @override
  void dispose() {
    _statusPoller?.cancel();
    _connectionStatusManager.dispose();
    _deviceAddedSub.cancel();
    _deviceRemovedSub.cancel();
    _client.close();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(body: Center(child: _buildContent(context)));
  }

  Widget _buildContent(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;

    if (!_connectionStatusManager.isConnected) {
      return Text('No connection to DBus service', style: textTheme.titleLarge);
    }

    if (devices.isEmpty) {
      return Text('No device connected', style: textTheme.titleLarge);
    }

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text('Connected', style: textTheme.titleLarge),
            Gap(8),
            Icon(Icons.check_circle_outline),
          ],
        ),
        Text(devices.first.friendlyName, style: textTheme.titleMedium),
        if (batteryLevel case int batteryLevel)
          Text('Battery: $batteryLevel%', style: textTheme.titleMedium),
      ],
    );
  }
}

class ConnectionStatusManager with ChangeNotifier {
  final DBusClient _client;

  StreamSubscription<DBusNameOwnerChangedEvent>? _subscription;
  bool _isConnected = false;

  ConnectionStatusManager(this._client);

  bool get isConnected => _isConnected;

  Future<void> init() async {
    _subscription?.cancel();

    _isConnected = await _client.nameHasOwner('dev.hasali.OpenGameSir');

    _subscription = _client.nameOwnerChanged.listen((event) {
      if (event.name == 'dev.hasali.OpenGameSir') {
        final isConnected = event.newOwner != null;
        if (_isConnected != isConnected) {
          _isConnected = isConnected;
          notifyListeners();
        }
      }
    });

    notifyListeners();
  }

  @override
  void dispose() {
    super.dispose();
    _subscription?.cancel();
  }
}
