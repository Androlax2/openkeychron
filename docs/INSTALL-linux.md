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
  keyboard, including ones that change the keymap, macros or lighting. That is what
  openkeychron needs, and also what any other program you run could do. VIA cannot reboot this
  firmware into its bootloader (`docs/DECISIONS.md`, #40).
- **Keystrokes can be exposed through the VIA channel itself.** The typing interface stays
  closed, but Keychron builds its firmware with `VIA_INSECURE`, which lets a plain read command
  report the keys held right now. See [Keystrokes and `VIA_INSECURE`](#keystrokes-and-via_insecure).
- **What it does not do:** no group changes, no world-readable nodes (`MODE="0666"`), and no
  access to Keychron devices other than those listed. Other models are added as they are
  tested (#41).

## Keystrokes and `VIA_INSECURE`

Opening the VIA channel is not neutral for privacy on Keychron firmware (#66).

**Confirmed from public source** (`Keychron/qmk_firmware` at
`2b493ff95f2665db719b1e95c91babb5036e72f3`):

- `keyboards/keychron/common/keychron_common.mk` line 1 adds `-DVIA_INSECURE` to every
  Keychron board that includes it; the V5 Max does (`keyboards/keychron/v5_max/rules.mk`
  line 4).
- `quantum/via.c` lines 25–27: QMK warns at build time, "VIA_INSECURE is enabled - firmware is
  susceptible to keyloggers".
- `quantum/via.c` line 331: with the flag, VIA's `id_get_keyboard_value` /
  `id_switch_matrix_state` (`02 03`) returns `matrix_get_row()`, the keys held at that moment;
  without it, zeros. Keychron's own handler intercepts this only on analog-matrix boards
  (`keyboards/keychron/common/keychron_raw_hid.c` line 133), not the V5 Max.
- It is a read: no write and no authentication. So **any program that can open the VIA
  node can poll which keys are pressed.** Who can open it is set by udev rules: this one,
  and any broader rule such as OpenRGB's below.

**Not verified on hardware:**

- whether the firmware on a given keyboard was built with `VIA_INSECURE` (release builds may
  differ from the public source);
- how fast the matrix can be polled, so how much typing could actually be reconstructed;
- whether it works through the Keychron Link receiver, where VIA itself is unverified (#20);
- which users and programs can open the VIA node on a given machine right now (check with
  `getfacl`, below).

Treat granting VIA access as granting the ability to watch keypresses, until a hardware test
shows otherwise.

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
