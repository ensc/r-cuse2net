# About

r-cuse2net implements a [CUSE](https://lwn.net/Articles/308445/)
device which relays operations on it to a remote server.

At the moment, only the most important functionality for TTY devices
is implemented.

It was implemented to make serial devices in podman containers available
and allow reconnecting them without restarting the container.

It consists of:

 - a server part (`cuse2net-dev`) which listens on the network and
   speaks with the real device. This program does not need special
   permission

 - a client part (`cuse2net-cuse`) which implements the cuse device and
   speaks with the server.  This program requires access to `/dev/cuse`
   (see SECURITY below)

# Usage

## cuse2net-dev server

```
Run character devices over network

Usage: cuse2net-dev [OPTIONS] --device <DEVICE>

Options:
      --log-format <FMT>  log format [default: default] [possible values: default, compact, full, json]
  -l, --listen <IP>       ip address to listen on [default: ::]
  -p, --port <PORT>       port to listen on [default: 8000]
  -d, --device <DEVICE>   device
  -h, --help              Print help
  -V, --version           Print version
```

## cuse2net-cuse client

Run character devices over network

```
Usage: cuse2net-cuse [OPTIONS] --server <server:port> --device <DEVICE>

Options:
      --log-format <FMT>      log format [default: default] [possible values: default, compact, full, json]
  -s, --server <server:port>  device major number
  -m, --major <node-major>    device major number
      --minor <node-minor>    device minor number
  -d, --device <DEVICE>       device name (without /dev)
  -h, --help                  Print help
  -V, --version               Print version
```

## Examples

### server

```
cuse2net-dev --device /dev/serial/by-path/pci-0000:00:14.0-usb-0:3:1.0-port0 --listen 127.0.0.1 --port 9001
```

### cuse

```
cuse2net-cuse --device ttyCUSE0 --major 450 --minor 100 --server 127.0.0.1:9000
```

or (when using the systemd services in [contrib/](contrib/)

```
# cat <<EOF >/etc/sysconfig/cuse2net-ttyCUSE0.conf
SERVER_IP=127.0.0.1
SERVER_PORT=9000
CUSE_MAJOR=450
CUSE_MINOR=100
# RUST_LOG=debug
EOF

# systemctl enable cuse2net-cuse@ttyCUSE0
```

### ESP32 IDF within podman

```
podman run --device=/dev/ttyCUSE0:rwm ...
esptool esp32 -p /dev/ttyCUSE0 ...
```

# Security

## /dev/cuse

`cuse2net-cuse` can run as an arbitrary user and needs only access to
`/dev/cuse`.  But in opposite to the related `/dev/fuse` device, it is
not recommended to open its permissions too much (e.g. **do not** make
it world accessible).

CUSE allows reading and writing of arbitrary memory of a program which
runs an `ioctl()` on it.

Hence, keep permissions of `/dev/cuse` restricted and run `cuse2net-cuse`
as a privileged user.


## `cuse2net-cuse`

Program should run as a privileged user (see above).  There is no
special risk regarding the CUSE related operations


## `cuse2net-dev`

Program needs access to the real device which can be accomplished by
special `udev` rules.

It runs `ioctl` requested from the client program.  At the moment only
a limited amount of ioctl are allowed, but when arbitrary ones are
possible, special crafted arguments might override memory and allow
attacks on the server.


## client program

As written above, the program that accesses the device generated by
`cuse2net-cuse` is defenseless at its mercy.


# TODO

- make `ioctl` on server side non-blocking

- implement more `ioctl`

- implement USB

- create an abstraction over ioctls; currently, it is expected that
  ioctl codes on server and client side are identical

# Supported clients

- ESP32 IDF
