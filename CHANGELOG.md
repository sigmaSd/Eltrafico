**1.7.0**
- Show programs live network usage using [nethogs](https://github.com/raboof/nethogs) as an optional dependency

**1.6.0**
- Switch form `ifstat` to `ifconfig -a` for enumerating interfaces, this fixes a crash where `ifstat` fails to detect the newly created `ifb` interface
- Add a root permission check

**1.5.0**
- Handle `ctrlc` signal (perform cleanup before shutdown)

**1.4.0**
- Add visual feedback

**1.3.0**
- Switch from `lsof` to `ss` and from `ifconfig` to `ifstat`so now `eltrafico` only depends on `iproute2` which is a core pkg as far as I know. Also add a check in case someone actually doesn't have it

**1.2.0**
`Gui` improvements:
  - App box is now vertically scroll-able
  - program names have the same width

**1.1.1**
Initial Release
