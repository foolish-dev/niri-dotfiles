use std::fs;

// ── NPU (AMD XDNA) detection ──────────────────────────────────────────────────

/// AMD's PCI vendor id.
const AMD_PCI_VENDOR: u16 = 0x1022;

/// Every PCI device id the in-tree `amdxdna` driver binds, copied verbatim from
/// its own module alias table so the two can't drift:
///
/// ```text
/// $ modinfo amdxdna | grep ^alias
/// alias: pci:v00001022d00001502sv*sd*bc*sc*i*   Phoenix / Hawk Point
/// alias: pci:v00001022d000017F0sv*sd*bc*sc*i*   Strix / Krackan / Strix Halo
/// alias: pci:v00001022d000017F2sv*sd*bc*sc*i*
/// alias: pci:v00001022d000017F3sv*sd*bc*sc*i*
/// alias: pci:v00001022d00001B0Bsv*sd*bc*sc*i*
/// alias: pci:v00001022d00001B0Csv*sd*bc*sc*i*
/// ```
///
/// The GZ302EA this repo was written on is `1022:17f0` rev 11 at `c5:00.1`.
const XDNA_PCI_DEVICE_IDS: [u16; 6] = [0x1502, 0x17f0, 0x17f2, 0x17f3, 0x1b0b, 0x1b0c];

/// Whether this host gets the AMD XDNA NPU userspace pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpuPack {
    /// An XDNA NPU function is on the PCI bus. Install XRT + FastFlowLM and
    /// write the memlock drop-in they need.
    Install,
    /// No XDNA function on this box. Skip all three steps — including the
    /// system-wide memlock policy, which exists only to serve them.
    Skip,
}

/// Decide from the enumerated PCI bus alone.
///
/// The gate is deliberately the *device*, not any of the four signals that look
/// equally good on the machine this was written on:
///
/// * `/sys/class/accel/accel0` — created when a driver binds, and the accel
///   class is shared: Intel's `ivpu` and `habanalabs` register there too, so an
///   Intel Core Ultra laptop would match and get AMD's runtime.
/// * `/sys/bus/pci/drivers/amdxdna/` and `amdxdna` in `/proc/modules` — both
///   only exist once the module is loaded. Blacklist it, boot a kernel built
///   without it, or probe before the modprobe settles and a box with the
///   hardware answers "no".
/// * The DMI product name (`ROG Flow Z13 GZ302EA_GZ302EA` here) — brittle in
///   both directions. It misses every other Phoenix/Strix machine with the same
///   silicon, and it matches a GZ302EA whose NPU is switched off in firmware.
///
/// The PCI vendor/device pair is the only one that answers the actual question.
/// The nodes under `/sys/bus/pci/devices/` are written by the PCI core at
/// enumeration, before and regardless of any driver claiming them, so on a host
/// where the driver exists but is not loaded — blacklisted, or a kernel too old
/// for it — this still returns `Install`. That is the wanted answer: the
/// `amdxdna` driver is *in-tree*, so "absent" here can only ever mean absent
/// hardware. The userspace is the only part this pack supplies, and it is
/// exactly what such a host is missing.
pub(crate) fn npu_pack(pci_devices: &[(u16, u16)]) -> NpuPack {
    let present = pci_devices
        .iter()
        .any(|&(vendor, device)| vendor == AMD_PCI_VENDOR && XDNA_PCI_DEVICE_IDS.contains(&device));
    if present {
        NpuPack::Install
    } else {
        NpuPack::Skip
    }
}

/// Parse one `/sys/bus/pci/devices/*/{vendor,device}` file body.
///
/// Pure so the format is pinned by tests rather than by the one box that has
/// the hardware: sysfs writes these as a lowercase `0x`-prefixed 4-digit hex
/// word plus a trailing newline (`"0x17f0\n"`). Both the prefix and the newline
/// are load-bearing — `from_str_radix` rejects either one on its own.
fn parse_pci_id(raw: &str) -> Option<u16> {
    let t = raw.trim();
    let hex = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X"))?;
    u16::from_str_radix(hex, 16).ok()
}

/// Which account the NPU memlock grant belongs to, given `$SUDO_USER` and
/// `$USER`.
///
/// `SUDO_USER` wins. dotctl is meant to run unprivileged and shell out to
/// `sudo` itself, but under a `sudo dotctl install` `$USER` is `root` — and
/// root is the single account that never needed the grant (`mlock` skips the
/// RLIMIT_MEMLOCK check entirely for CAP_IPC_LOCK), while the human who will
/// actually run FastFlowLM would get nothing. Empty is treated as unset: a
/// display manager or an `env -u` can hand down `USER=`.
pub(crate) fn memlock_user<'a>(
    sudo_user: Option<&'a str>,
    user: Option<&'a str>,
) -> Option<&'a str> {
    [sudo_user, user]
        .into_iter()
        .flatten()
        .find(|u| !u.is_empty() && *u != "root")
}

/// Body of `/etc/security/limits.d/99-amdxdna.conf`.
///
/// Scoped to one account rather than the `*` this used to write. pam_limits
/// applies limits.d to PAM *login sessions*, so `*` never did loosen system
/// daemons — systemd units take `DefaultLimitMEMLOCK` (8 MiB here) and never
/// consult PAM at all — but it did hand unlimited locked memory to every human
/// login on the box, and mlock'd pages are unswappable and unreclaimable, so
/// that 8 MiB default is the only thing standing between an unprivileged
/// account and pinning all of RAM. One user is all this workload needs.
///
/// Not a group: the obvious candidate, `@render`, is wrong twice over —
/// udev's `50-udev-default.rules` publishes the accel node as `0666`, so group
/// membership is not what gates NPU access, and the desktop user is typically
/// not in `render` anyway.
pub(crate) fn amdxdna_memlock_conf(user: &str) -> String {
    format!(
        "# Written by `dotctl install` for the AMD XDNA NPU.\n\
         # XRT pins the host-side DMA buffers it hands the NPU, and the default\n\
         # RLIMIT_MEMLOCK is 8 MiB — three orders of magnitude under a multi-GB\n\
         # FastFlowLM model, so without this a model load dies with ENOMEM.\n\
         # `-` sets the soft and hard limit together. Unlimited rather than a\n\
         # figure because the ceiling is the model, not the runtime: XRT\n\
         # documents no bound to size a number against.\n\
         {user}  -  memlock  unlimited\n"
    )
}

/// Every PCI function the kernel enumerated, as `(vendor, device)`.
///
/// sysfs rather than `lspci`: pciutils is not installed by dotctl and is not a
/// dependency of anything it has installed by this point, so shelling out to
/// `lspci` would turn a missing optional tool into a false "no NPU". Reading
/// the same numbers straight out of `/sys` needs nothing but the kernel, and
/// skips parsing a human-readable name that changes with every `pci.ids` bump.
///
/// An unreadable `/sys/bus/pci` yields an empty list, which reads as "no NPU" —
/// the safe direction: we skip an optional hardware pack rather than install a
/// runtime and a system-wide limits policy on a guess.
pub(crate) fn pci_devices() -> Vec<(u16, u16)> {
    let Ok(entries) = fs::read_dir("/sys/bus/pci/devices") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let field = |name: &str| {
                fs::read_to_string(e.path().join(name))
                    .ok()
                    .and_then(|s| parse_pci_id(&s))
            };
            Some((field("vendor")?, field("device")?))
        })
        .collect()
}

// ── ASUS ROG platform detection ───────────────────────────────────────────────

/// The udev sysname of the platform device the in-tree `asus-nb-wmi` driver
/// registers, and the exact string asusctl's own `rog-platform` crate hunts for
/// before it will do anything (`rog-platform/src/platform.rs`):
///
/// ```text
/// enumerator.match_subsystem("platform")?;
/// enumerator.match_sysname("asus-nb-wmi")?;
/// ...
/// Err(PlatformError::MissingFunction("asus-nb-wmi not found".into()))
/// ```
///
/// Everything rog-control-center draws is a read or a write of an attribute on
/// that device — on this GZ302EA `throttle_thermal_policy`, `panel_od`,
/// `ppt_pl1_spl`, `ppt_pl2_sppt`, `ppt_fppt`, `nv_temp_target` and
/// `charge_mode`, plus its `platform-profile`, `hwmon`, `leds` and `rfkill`
/// children.
const ASUS_PLATFORM_DEVICE: &str = "asus-nb-wmi";

/// Whether this host gets the ASUS ROG control pack — `rog-control-center` and
/// the `asusctl` daemon it is a front end for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AsusPack {
    /// `asus-nb-wmi` is bound, so asusd has the device it drives and the
    /// Armoury Crate key has something to open.
    Install,
    /// No ASUS notebook WMI device. Skip it: asusd would exit with
    /// `MissingFunction` and the GUI would open onto nothing.
    Skip,
}

/// Decide from the platform bus alone.
///
/// Deliberately *not* DMI, even though DMI is the natural home for a vendor's
/// identity and this pack really is vendor firmware rather than a PCI function.
/// `/sys/class/dmi/id` on this box says `sys_vendor=ASUSTeK COMPUTER INC.`,
/// `chassis_vendor=ASUSTeK COMPUTER INC.`,
/// `product_name=ROG Flow Z13 GZ302EA_GZ302EA`, `board_name=GZ302EA` — and
/// every one of those answers a question we are not asking:
///
/// * `sys_vendor` / `chassis_vendor` — "was this box built by ASUS", which is
///   equally true of every ASUS desktop and of anyone else's case with an ASUS
///   motherboard in it. Those publish the desktop sensor interfaces
///   (`asus_wmi_sensors`, `asus-ec-sensors`), not the notebook one, so asusd
///   would install, fail to find its device and the GUI would open onto
///   nothing. The string is unstable too — firmware ships `ASUSTeK COMPUTER
///   INC.`, `ASUSTeK Computer Inc.` and bare `ASUS` across generations — so the
///   match would have to be case-folded and prefix-y, widening it further.
/// * `product_name` / `board_name` — "is this the author's laptop". It misses
///   every other ROG and Zenbook asusctl supports, which is the entire reason
///   to install a vendor daemon instead of hard-coding this machine.
///
/// The platform device answers the question that actually matters — *will this
/// daemon do anything here* — because it is the same question asusd asks
/// itself, in the same words.
///
/// This is the opposite call from [`npu_pack`], which rejected every
/// driver-bound signal, and the two are consistent rather than in tension.
/// There the pack ships the *userspace* for an in-tree driver, so a host with
/// the silicon and an unloaded module still needs the packages and a
/// driver-bound gate would have wrongly said no. Here the pack's entire value
/// arrives *through* that driver's sysfs attributes, so a host where
/// `asus-nb-wmi` is not bound — blacklisted, a kernel without it, an ASUS board
/// that has no notebook WMI to bind — is a host where installing asusctl buys
/// nothing. "The driver isn't bound" is the right answer here, not a false
/// negative to engineer around.
///
/// The runner-up was the firmware-published GUID itself,
/// `/sys/bus/wmi/devices/0B3CBB35-E3C2-45ED-91C2-4C5A6D195D1C-1` — the sole
/// modalias of `asus-nb-wmi` (`modinfo asus-nb-wmi`), created by the WMI bus
/// core out of the ACPI `_WDG` table before any driver claims it, and so the
/// closest thing on this bus to the pre-driver PCI enumeration `npu_pack`
/// leans on. It loses on the same ground: a GUID sitting there with nothing
/// bound to it is exactly the case where the pack does nothing, so the extra
/// purity would only buy false `Install`s.
pub(crate) fn asus_pack(platform_devices: &[String]) -> AsusPack {
    if platform_devices.iter().any(|d| d == ASUS_PLATFORM_DEVICE) {
        AsusPack::Install
    } else {
        AsusPack::Skip
    }
}

/// Every device name on the platform bus.
///
/// `/sys/bus/platform/devices` rather than `/sys/devices/platform`: the bus
/// directory is exactly the set udev's `match_subsystem("platform")` walks, so
/// it is the same view asusd has, and it also carries platform devices parented
/// elsewhere in the device tree, which the `/sys/devices/platform` listing does
/// not — 50 entries against 36 on this machine.
///
/// An unreadable bus yields an empty list, which reads as "no ASUS platform" —
/// the safe direction, the same one [`pci_devices`] takes: forgo an optional
/// vendor pack rather than install a system daemon on a guess.
pub(crate) fn platform_devices() -> Vec<String> {
    let Ok(entries) = fs::read_dir("/sys/bus/platform/devices") else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── AMD XDNA NPU detection ───────────────────────────────────────────

    /// The `(vendor, device)` pairs `/sys/bus/pci/devices` reports on the ROG
    /// Flow Z13 GZ302EA this pack was written for, in bus order. The NPU is the
    /// second-to-last entry (`1022:17f0` at `c5:00.1`), so a test using this
    /// list also proves the scan doesn't only look at the head of the bus.
    const GZ302EA_PCI_BUS: [(u16, u16); 12] = [
        (0x1022, 0x1507), // Strix Halo root complex
        (0x1022, 0x1508), // IOMMU
        (0x1022, 0x150a), // USB4 bridge
        (0x1022, 0x790b), // FCH SMBus
        (0x17a0, 0x9755), // Genesys SD host controller
        (0x14c3, 0x7925), // MediaTek MT7925 wifi
        (0x15b7, 0x5034), // Sandisk NVMe
        (0x1002, 0x1586), // Radeon 8060S
        (0x1022, 0x17e0), // CCP/ASP
        (0x1022, 0x164a), // Sensor Fusion Hub — also a 0x118000 signal processor
        (0x1022, 0x17f0), // the NPU
        (0x1022, 0x1588), // USB 3.1 xHCI
    ];

    #[test]
    fn npu_pack_installs_on_the_machine_this_pack_was_written_for() {
        // Pins the whole detection against the real GZ302EA bus dump, including
        // that the NPU is found in the middle of a twelve-function listing.
        assert_eq!(npu_pack(&GZ302EA_PCI_BUS), NpuPack::Install);
    }

    #[test]
    fn npu_pack_installs_for_every_device_id_the_amdxdna_driver_binds() {
        // Exhaustive over the driver's own alias table: any id amdxdna would
        // claim must get the userspace, or the pack silently misses a Phoenix,
        // Hawk Point, Krackan or Strix machine that has the hardware.
        for id in XDNA_PCI_DEVICE_IDS {
            assert_eq!(
                npu_pack(&[(AMD_PCI_VENDOR, id)]),
                NpuPack::Install,
                "{id:#06x} is in the amdxdna alias table"
            );
        }
    }

    #[test]
    fn npu_pack_skips_a_host_with_no_npu_function_on_the_bus() {
        // The case that used to abort install() and take deploy() down with it:
        // an ordinary AMD desktop, every function present except the NPU.
        let bus: Vec<(u16, u16)> = GZ302EA_PCI_BUS
            .into_iter()
            .filter(|&(_, d)| d != 0x17f0)
            .collect();
        assert_eq!(npu_pack(&bus), NpuPack::Skip);
    }

    #[test]
    fn npu_pack_skips_an_intel_laptop_that_also_registers_an_accel_device() {
        // Why the gate is the PCI id and not /sys/class/accel: Intel's `ivpu`
        // publishes accel0 for its own NPU. Matching on the accel class would
        // install AMD's XRT and a memlock policy on a Core Ultra box.
        assert_eq!(
            npu_pack(&[(0x8086, 0x7d1d), (0x8086, 0x643e), (0x8086, 0xa76f)]),
            NpuPack::Skip
        );
    }

    #[test]
    fn npu_pack_does_not_match_an_xdna_device_id_from_another_vendor() {
        // PCI device ids are only unique per vendor: 0x17f0 belongs to some
        // other part on some other vendor's bus. The pair has to match.
        for id in XDNA_PCI_DEVICE_IDS {
            assert_eq!(npu_pack(&[(0x8086, id)]), NpuPack::Skip, "{id:#06x}");
            assert_eq!(npu_pack(&[(0x10de, id)]), NpuPack::Skip, "{id:#06x}");
        }
    }

    #[test]
    fn npu_pack_skips_when_the_pci_bus_could_not_be_read_at_all() {
        // `pci_devices()` answers with an empty Vec when /sys/bus/pci is
        // unreadable. Skip is the safe direction: forgo an optional hardware
        // pack rather than write a system-wide limits policy on a guess.
        assert_eq!(npu_pack(&[]), NpuPack::Skip);
    }

    #[test]
    fn parse_pci_id_reads_the_exact_spelling_sysfs_uses() {
        // sysfs writes `0x17f0\n` — the prefix and the trailing newline are
        // both load-bearing, and `from_str_radix` rejects either one alone.
        assert_eq!(parse_pci_id("0x17f0\n"), Some(0x17f0));
        assert_eq!(parse_pci_id("0x1022\n"), Some(0x1022));
        assert_eq!(parse_pci_id("0x1502"), Some(0x1502));
        assert_eq!(parse_pci_id("0X17F0\n"), Some(0x17f0));
        assert_eq!(parse_pci_id("  0x17f0  "), Some(0x17f0));
    }

    #[test]
    fn parse_pci_id_rejects_anything_that_is_not_a_sysfs_pci_id() {
        // A bare number must NOT parse: reading `17f0` as decimal, or an empty
        // sysfs read as 0, would hand npu_pack a device id that never existed.
        assert_eq!(parse_pci_id("17f0\n"), None);
        assert_eq!(parse_pci_id(""), None);
        assert_eq!(parse_pci_id("0x"), None);
        assert_eq!(parse_pci_id("0xzzzz"), None);
        assert_eq!(parse_pci_id("0x117f0"), None); // wider than u16
    }

    #[test]
    fn the_real_pci_bus_scan_parses_every_function_the_kernel_enumerated() {
        // The impure half against real sysfs. Every PCI function Linux
        // enumerates has readable `vendor` and `device` nodes, so a count short
        // of the directory listing means `parse_pci_id` silently dropped
        // something — and a dropped entry is exactly how a host with the NPU
        // would get told it has none. Skipped where /sys/bus/pci is absent
        // (containers, non-x86 CI), which the empty-bus test already covers.
        let dir = Path::new("/sys/bus/pci/devices");
        if !dir.is_dir() {
            return;
        }
        let enumerated = fs::read_dir(dir)
            .expect("read /sys/bus/pci/devices")
            .count();
        assert_eq!(pci_devices().len(), enumerated);
    }

    // ── ASUS ROG platform detection ──────────────────────────────────────

    /// The device names `/sys/bus/platform/devices` reports on the ROG Flow Z13
    /// GZ302EA this pack was written for, in listing order. `asus-nb-wmi` sits
    /// in the middle of it, so a test over this list also proves the scan does
    /// not just look at the head of the bus. Trimmed to a representative slice
    /// of the 50 entries; every name here is one this machine really publishes.
    const GZ302EA_PLATFORM_BUS: [&str; 10] = [
        "acp_asoc_acp70.0",
        "acpi-cpufreq",
        "ACPI0011:00",
        "AMDI0010:03",
        "asus-nb-wmi",
        "ASUS2018:00",
        "ASUS9001:00",
        "i8042",
        "PNP0C0C:00",
        "serial8250",
    ];

    /// `asus_pack` takes owned names because that is what a sysfs listing
    /// yields; the tests would rather write string literals.
    fn platform_bus(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_string()).collect()
    }

    #[test]
    fn asus_pack_installs_on_the_machine_this_pack_was_written_for() {
        // Pins the detection against the real GZ302EA platform bus, including
        // that asus-nb-wmi is found in the middle of the listing.
        assert_eq!(
            asus_pack(&platform_bus(&GZ302EA_PLATFORM_BUS)),
            AsusPack::Install
        );
    }

    #[test]
    fn asus_pack_skips_a_host_with_no_asus_notebook_wmi_device() {
        // The case this gate exists for: every other platform device present,
        // asus-nb-wmi absent, so asusd would start and immediately fail with
        // MissingFunction("asus-nb-wmi not found").
        let bus: Vec<String> = platform_bus(&GZ302EA_PLATFORM_BUS)
            .into_iter()
            .filter(|d| d != ASUS_PLATFORM_DEVICE)
            .collect();
        assert_eq!(asus_pack(&bus), AsusPack::Skip);
    }

    #[test]
    fn asus_pack_skips_an_asus_built_desktop_that_publishes_only_sensor_devices() {
        // Why the gate is not DMI: this host's sys_vendor and chassis_vendor
        // are the same "ASUSTeK COMPUTER INC." string a desktop board reports.
        // What a desktop board does not have is the notebook WMI interface —
        // it publishes the hwmon drivers instead — so asusctl there is a daemon
        // with nothing to drive.
        assert_eq!(
            asus_pack(&platform_bus(&[
                "asus_wmi_sensors",
                "asus-ec-sensors",
                "i8042"
            ])),
            AsusPack::Skip
        );
    }

    #[test]
    fn asus_pack_skips_an_asus_laptop_whose_wmi_is_claimed_by_a_different_driver() {
        // eeepc-wmi binds the same ASUS management GUID asus-nb-wmi does and
        // registers its own platform device instead. asusctl looks for exactly
        // one sysname and would find nothing, so Skip is the honest answer even
        // though the box is an ASUS laptop with ASUS WMI firmware.
        assert_eq!(
            asus_pack(&platform_bus(&["eeepc-wmi", "eeepc-laptop"])),
            AsusPack::Skip
        );
    }

    #[test]
    fn asus_pack_matches_the_platform_device_name_exactly() {
        // Not a substring or prefix match. `asus_nb_wmi` in particular is the
        // *module* name (underscores, as /proc/modules and lsmod print it); the
        // device sysname asusctl queries is hyphenated, and matching either
        // spelling would let a module-loaded proxy back in through the door
        // this gate deliberately closes.
        for name in [
            "asus_nb_wmi",
            "asus-nb-wmi:00",
            "asus-nb-wmi.0",
            "xasus-nb-wmi",
            "asus-wmi",
        ] {
            assert_eq!(asus_pack(&platform_bus(&[name])), AsusPack::Skip, "{name}");
        }
    }

    #[test]
    fn asus_pack_skips_when_the_platform_bus_could_not_be_read_at_all() {
        // `platform_devices()` answers with an empty Vec when /sys/bus/platform
        // is unreadable. Skip is the safe direction: forgo an optional vendor
        // pack rather than install a system daemon on a guess.
        assert_eq!(asus_pack(&[]), AsusPack::Skip);
    }

    #[test]
    fn the_real_platform_bus_scan_names_every_device_the_kernel_enumerated() {
        // The impure half against real sysfs, mirroring the PCI scan test
        // above: a count short of the directory listing means a name was
        // silently dropped, and a dropped name is exactly how a ROG laptop
        // would get told it is not one. Skipped where /sys/bus/platform is
        // absent (containers, some CI), which the empty-bus test already covers.
        let dir = Path::new("/sys/bus/platform/devices");
        if !dir.is_dir() {
            return;
        }
        let enumerated = fs::read_dir(dir)
            .expect("read /sys/bus/platform/devices")
            .count();
        assert_eq!(platform_devices().len(), enumerated);
    }

    #[test]
    fn memlock_user_prefers_the_human_behind_a_sudo_invocation() {
        // dotctl calls sudo itself, but under `sudo dotctl install` $USER is
        // root — writing the grant for root leaves the account that actually
        // runs FastFlowLM at the 8 MiB default.
        assert_eq!(memlock_user(Some("fool"), Some("root")), Some("fool"));
    }

    #[test]
    fn memlock_user_takes_the_login_user_when_sudo_user_is_unset() {
        // The normal path: dotctl run unprivileged, SUDO_USER absent or empty.
        assert_eq!(memlock_user(None, Some("fool")), Some("fool"));
        assert_eq!(memlock_user(Some(""), Some("fool")), Some("fool"));
    }

    #[test]
    fn memlock_user_declines_to_write_a_grant_for_root() {
        // mlock skips the RLIMIT_MEMLOCK check outright for CAP_IPC_LOCK, so a
        // root entry buys nothing; None makes the caller say so instead of
        // writing a misleading line into /etc.
        assert_eq!(memlock_user(Some("root"), Some("root")), None);
        assert_eq!(memlock_user(None, Some("root")), None);
        assert_eq!(memlock_user(None, None), None);
        assert_eq!(memlock_user(Some(""), Some("")), None);
    }

    #[test]
    fn amdxdna_memlock_conf_grants_exactly_one_named_account() {
        // The bug this replaced: a `*` domain handed unlimited locked memory to
        // every login on the box, on hardware most of them do not have.
        let body = amdxdna_memlock_conf("fool");
        let rules: Vec<&str> = body
            .lines()
            .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
            .collect();
        assert_eq!(rules, ["fool  -  memlock  unlimited"]);
        assert!(!body.contains('*'), "no wildcard domain: {body}");
    }

    #[test]
    fn amdxdna_memlock_conf_says_what_the_file_is_for() {
        // pam_limits ignores everything after a `#`, so the explanation the next
        // reader needs lives in the file itself rather than only in this repo.
        let body = amdxdna_memlock_conf("fool");
        for word in ["XRT", "8 MiB", "ENOMEM", "dotctl install"] {
            assert!(body.contains(word), "{word:?} missing from:\n{body}");
        }
        assert!(
            body.ends_with('\n'),
            "pam_limits wants a terminated last line"
        );
    }
}
