# Linux: letting openkeychron reach the keyboard

On Linux, a normal user cannot open `/dev/hidraw*` nodes. openkeychron needs one of them: the
keyboard's VIA raw HID channel. [`packaging/linux/70-openkeychron.rules`](../packaging/linux/70-openkeychron.rules)
opens exactly that channel to the logged-in user, and nothing else.

## What the rule opens, and to whom

| Device | USB ID | Interface opened | Interfaces left closed |
|---|---|---|---|
| Keychron Link 2.4 GHz receiver | `3434:d030` | 3, the VIA channel | 0 mouse, 1 the receiver's own channel, 2 typing |
| Keychron V5 Max ISO, USB cable | `3434:0951` | 1, the VIA channel (from QMK source; not yet seen on hardware, #20) | all others, including typing |

- **Who:** `TAG+="uaccess"` gives read and write access to the user of the active local
  session (systemd-logind), and only while that session is active. Other users, SSH sessions
  and a locked-out seat get nothing.
- **What that allows:** any program running as that user can send VIA commands to the
  keyboard, including ones that change the keymap, macros or lighting, or reboot it into its
  bootloader. That is what openkeychron needs, and also what any other program you run could
  do. Keystrokes are not exposed: the typing interface stays closed.
- **What it does not do:** no group changes, no world-readable nodes (`MODE="0666"`), and no
  access to Keychron devices other than those listed. Other models are added as they are
  tested (#41).

## Before you install: check for broader rules

Other software may already open every Keychron interface. For example, OpenRGB's
`60-openrgb.rules` contains:

```
SUBSYSTEMS=="usb|hidraw", ATTRS{idVendor}=="3434", TAG+="uaccess", …
```

That opens **all** interfaces of every Keychron device, typing included, to the logged-in
user. With it installed, this rule adds nothing; removing the broader rule is your decision.
To list rules that mention Keychron:

```sh
grep -rl -i '3434' /etc/udev/rules.d /usr/lib/udev/rules.d
```

## Install

Read the rule first. Then:

```sh
sudo install -m 0644 packaging/linux/70-openkeychron.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=hidraw --action=change
```

Unplug and replug the receiver (or the keyboard cable) if the access does not change.

## Check

```sh
for node in /sys/class/hidraw/hidraw*; do
  printf '%s ' "${node##*/}"
  udevadm test-builtin usb_id "$node" 2>/dev/null |
    grep -E '^ID_(VENDOR_ID|MODEL_ID|USB_INTERFACE_NUM)=' | tr '\n' ' '
  echo
done
getfacl /dev/hidraw*   # the opened node lists your user with rw
```

## Remove

```sh
sudo rm /etc/udev/rules.d/70-openkeychron.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=hidraw --action=change
```

## Without systemd-logind

`uaccess` needs systemd-logind. Elsewhere, replace `TAG+="uaccess"` with
`MODE="0660", GROUP="<a group only you are in>"`, and add your user to that group.

## Only one program at a time

Close Chrome, Keychron Launcher and VIA before using openkeychron: the keyboard answers one
program's VIA session at a time, and every program reading the node sees every reply, so two
sessions would confuse each other.
