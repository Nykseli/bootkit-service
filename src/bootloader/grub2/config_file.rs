use std::{
    fs::read_to_string,
    path::{Path, PathBuf},
};

use crate::{
    bootloader::parser::{ConfigFile, ConfigFileParser, FileLine, KeyValue},
    dctx,
    errors::{DError, DRes, DResult},
};

fn parse_config_line(line_num: usize, line: &str) -> DResult<KeyValue> {
    // TODO: save the type of quotes so they can be returned to orignal
    let trimmed = line.trim();
    let split = if let Some(split) = trimmed.split_once('=') {
        split
    } else {
        return Err(DError::grub_parse_error(
            dctx!(),
            format!("Expected '=' on line: {}", line_num + 1),
        ));
    };
    let key = split.0.trim().to_string();
    let value = split.1.trim().replace(['\'', '"'], "");

    Ok(KeyValue::new(line_num, line, key, value))
}

#[derive(Debug)]
pub struct Grub2ConfigFile {
    file: ConfigFile,
}

impl Grub2ConfigFile {
    pub fn new<P: Into<PathBuf>>(path: P, file: &str) -> DResult<Self> {
        let mut lines = Vec::new();

        // use split instead of lines to save the trailing empty new line
        // this doesn't handle \r\n but this is very unlikely to run on
        // windows anyways
        for (idx, line) in file.split('\n').enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                lines.push(FileLine::String {
                    raw_line: line.into(),
                });
                continue;
            }

            let keyval =
                parse_config_line(idx, line).ctx(dctx!(), "Failed to parse grub2 config file")?;
            lines.push(FileLine::KeyValue(keyval));
        }

        Ok(Self {
            file: ConfigFile::new(path, lines),
        })
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> DResult<Self> {
        let file = read_to_string(path.as_ref())
            .ctx(dctx!(), format!("Error reading {:?}", path.as_ref()))?;
        Self::new(path.as_ref(), &file)
    }

    pub fn timeout(&self) -> Option<String> {
        self.file
            .get_key_value("GRUB_TIMEOUT")
            .map(|kv| kv.value.to_string())
    }

    pub fn set_timeout<T: Into<String>>(&mut self, timeout: T) {
        self.update_or_insert("GRUB_TIMEOUT", timeout);
    }

    pub fn kernel_arguments(&self) -> Option<String> {
        self.file
            .get_key_value("GRUB_CMDLINE_LINUX_DEFAULT")
            .map(|kv| kv.value.to_string())
    }

    pub fn set_kernel_arguments<K: Into<String>>(&mut self, kernel: K) {
        self.update_or_insert("GRUB_CMDLINE_LINUX_DEFAULT", kernel);
    }
}

impl ConfigFileParser for Grub2ConfigFile {
    fn format_key_value(value: &KeyValue) -> String {
        format!("{}=\"{}\"", value.key, value.value)
    }

    fn config_file(&self) -> &ConfigFile {
        &self.file
    }

    fn config_file_mut(&mut self) -> &mut ConfigFile {
        &mut self.file
    }
}

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;

    use super::*;

    #[test]
    fn test_grub2_parsing_no_eol() {
        let file = Grub2ConfigFile::new("test", "GRUB_DEFAULT=saved").unwrap();
        let lines = file.lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], ("GRUB_DEFAULT", "saved"));
    }

    #[test]
    fn test_grub2_parsing_with_eol() {
        let file = Grub2ConfigFile::new("test", "GRUB_DEFAULT=saved\n").unwrap();
        let lines = file.lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], ("GRUB_DEFAULT", "saved"));
        // make sure the last line is empty (empty trailing line)
        assert_eq!(lines[1], "");
    }

    #[test]
    fn test_grub2_parsing_fail() {
        let err = Grub2ConfigFile::new("test", "GRUB_DEFAULT").unwrap_err();
        assert_eq!(
            err.error().as_string(),
            "Internal Parse: Failed to parse grub config: Expected '=' on line: 1"
        );
    }

    #[test]
    fn test_grub2_parsing_simple() {
        let file_data = read_to_string("test_data/grub_simple").unwrap();
        let file = Grub2ConfigFile::new("test", &file_data).unwrap();
        let lines = file.lines();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], ("GRUB_DISTRIBUTOR", ""));
        assert_eq!(lines[1], ("GRUB_DEFAULT", "saved"));
        assert_eq!(lines[2], ("GRUB_HIDDEN_TIMEOUT_QUIET", "true"));
        assert_eq!(lines[3], ("GRUB_TIMEOUT", "8"));
        // make sure the last line is empty (empty trailing line)
        assert_eq!(lines[4], "");
        assert_eq!(file.as_string(), file_data);
    }

    #[test]
    fn test_grub2_parsing_full() {
        let file_data = read_to_string("test_data/grub_full").unwrap();
        let file = Grub2ConfigFile::new("test", &file_data).unwrap();
        let lines = file.lines();
        assert_eq!(lines.len(), 46);
        assert_eq!(lines[0], "# If you change this file, run \'grub2-mkconfig -o /boot/grub2/grub.cfg\' afterwards to update");
        assert_eq!(lines[1], "# /boot/grub2/grub.cfg.");
        assert_eq!(lines[2], "");
        assert_eq!(lines[3], "# Uncomment to set your own custom distributor. If you leave it unset or empty, the default");
        assert_eq!(
            lines[4],
            "# policy is to determine the value from /etc/os-release"
        );
        assert_eq!(lines[5], ("GRUB_DISTRIBUTOR", ""));
        assert_eq!(lines[6], ("GRUB_DEFAULT", "saved"));
        assert_eq!(lines[7], ("GRUB_HIDDEN_TIMEOUT", "0"));
        assert_eq!(lines[8], ("GRUB_HIDDEN_TIMEOUT_QUIET", "true"));
        assert_eq!(lines[9], ("GRUB_TIMEOUT", "8"));
        assert_eq!(
            lines[10],
            (
                "GRUB_CMDLINE_LINUX_DEFAULT",
                "splash=silent quiet security=apparmor amd_pstate=active mitigations=auto"
            )
        );
        assert_eq!(lines[11], ("GRUB_CMDLINE_LINUX", ""));
        assert_eq!(lines[12], "");
        assert_eq!(
            lines[13],
            "# Uncomment to automatically save last booted menu entry in GRUB2 environment"
        );
        assert_eq!(lines[14], "");
        assert_eq!(lines[15], "# variable `saved_entry\'");
        assert_eq!(lines[16], "# GRUB_SAVEDEFAULT=\"true\"");
        assert_eq!(
            lines[17],
            "#Uncomment to enable BadRAM filtering, modify to suit your needs"
        );
        assert_eq!(lines[18], "");
        assert_eq!(
            lines[19],
            "# This works with Linux (no patch required) and with any kernel that obtains"
        );
        assert_eq!(
            lines[20],
            "# the memory map information from GRUB (GNU Mach, kernel of FreeBSD ...)"
        );
        assert_eq!(
            lines[21],
            "# GRUB_BADRAM=\"0x01234567,0xfefefefe,0x89abcdef,0xefefefef\""
        );
        assert_eq!(
            lines[22],
            "#Uncomment to disable graphical terminal (grub-pc only)"
        );
        assert_eq!(lines[23], "");
        assert_eq!(lines[24], ("GRUB_TERMINAL", "gfxterm"));
        assert_eq!(lines[25], "# The resolution used on graphical terminal");
        assert_eq!(
            lines[26],
            "#note that you can use only modes which your graphic card supports via VBE"
        );
        assert_eq!(lines[27], "");
        assert_eq!(
            lines[28],
            "# you can see them in real GRUB with the command `vbeinfo\'"
        );
        assert_eq!(lines[29], ("GRUB_GFXMODE", "auto"));
        assert_eq!(
            lines[30],
            "# Uncomment if you don\'t want GRUB to pass \"root=UUID=xxx\" parameter to Linux"
        );
        assert_eq!(lines[31], "# GRUB_DISABLE_LINUX_UUID=true");
        assert_eq!(
            lines[32],
            "#Uncomment to disable generation of recovery mode menu entries"
        );
        assert_eq!(lines[33], "");
        assert_eq!(lines[34], "# GRUB_DISABLE_RECOVERY=\"true\"");
        assert_eq!(lines[35], "#Uncomment to get a beep at grub start");
        assert_eq!(lines[36], "");
        assert_eq!(lines[37], "# GRUB_INIT_TUNE=\"480 440 1\"");
        assert_eq!(lines[38], ("GRUB_BACKGROUND", ""));
        assert_eq!(
            lines[39],
            ("GRUB_THEME", "/boot/grub2/themes/openSUSE/theme.txt")
        );
        assert_eq!(lines[40], ("SUSE_BTRFS_SNAPSHOT_BOOTING", "true"));
        assert_eq!(lines[41], ("GRUB_USE_LINUXEFI", "true"));
        assert_eq!(lines[42], ("GRUB_DISABLE_OS_PROBER", "false"));
        assert_eq!(lines[43], ("GRUB_ENABLE_CRYPTODISK", "y"));
        assert_eq!(
            lines[44],
            ("GRUB_CMDLINE_XEN_DEFAULT", "vga=gfx-1024x768x16")
        );
        assert_eq!(lines[45], "");

        assert_eq!(file.as_string(), file_data);
    }
}
