**2.3.1**
- Default to a working limits
- Fix a couple of warnings 

**2.3.0**
- Ui improvements

**2.2.4**
- Update dependencies
- Fix command line argument handling

**2.2.3**
- Fix a bug where limits werent applied

**2.2.2**
- Update dependencies

**2.2.1**
- Add [bandwhich](https://github.com/imsnif/bandwhich) as a netmonitoring backend
- UI improvement (float precision = 2 for download/upload)

**2.1.1**
- Parse `/proc/net/dev` manually instead of using `ifconfig` because of portability issues

**2.1.0**
- Add prcedence to --eltrafico-tc over $PATH
- Check if nethogs is running before using pkill
- Improve error handling, code cleanup

**2.0.0**

- Eltrafico is now split on 2 binaries:

  1- `eltrafico`: create gui and call `nethogs` and `eltrafico_tc` as privileged process using `pkexec`
  
  2- `eltrafico_tc`: traffic shaping, can be controlled via stdin, for the list of commands see https://github.com/sigmaSd/Eltrafico/blob/sudo_isolation/src/eltrafico_tc/main.rs#L252 and https://github.com/sigmaSd/Eltrafico/blob/sudo_isolation/src/eltrafico_tc/main.rs#L79

  This allows to run the gui as a normal user, and ask for higher privilege only for `eltrafico_tc` and `nethogs` binaries

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
