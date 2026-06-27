# SocketCAN

SocketCAN is the normal Linux interface for CAN devices and virtual CAN
interfaces.

For local smoke testing, use `vcan0` where possible. For real machinery, use
the correct interface, bitrate, termination, and safety procedure.

Typical setup outside this book:

```sh
sudo modprobe vcan
sudo ip link add dev vcan0 type vcan
sudo ip link set up vcan0
```

Real machine warning: never attach experimental software to a working machine
without a safe test plan, isolation, and permission from the equipment owner.
