//! `ios list` 的设备列表解析。
//!
//! 这些函数原先住在 GUI 的 `src-tauri/src/lib.rs`（P1 写的）。P4.1 把等价实现
//! 搬到这里，GUI 本片**保持原样**，P4.2 再切依赖并删掉那边的副本。
//!
//! 刻意**不引 serde_json 解析设备列表**：真实输出只需要取一个数组字段，手写提取
//! 反而能同时吃下 JSON 形状和历史的行式输出，而后者不是合法 JSON。版本号那种
//! 字段多的响应才用 serde_json（见 [`crate::parse_version`]、[`crate::wda`]）。

/// 界面用的 iOS 设备条目。
///
/// `id` 是完整 UDID，只在进程内流转；`name` 是给界面看的短名。
/// **两者都不持久化**（红线：设备标识不入仓库、不落盘）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IosDevice {
    pub id: String,
    pub name: String,
}

/// 解析 `ios list` 的 stdout。
///
/// 先按 `{"deviceList":[...]}` 取；不是这个形状就退回行式解析（旧版 go-ios 会打
/// `List of attached devices:` 开头的纯文本）。任何一个 UDID 为空或含空白都判为
/// 无法解析——宁可报错，也不要把半截输出当成设备列表。
pub fn parse_ios_devices(output: &str) -> Result<Vec<IosDevice>, String> {
    let udids = if let Some(list) = extract_device_list(output) {
        list
    } else {
        output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter(|line| !line.eq_ignore_ascii_case("list of attached devices:"))
            .map(|line| line.strip_prefix("UDID:").unwrap_or(line).trim().to_owned())
            .collect()
    };

    udids
        .into_iter()
        .map(|udid| {
            if udid.is_empty() || udid.contains(char::is_whitespace) {
                return Err("go-ios returned an unparsable device list".to_owned());
            }
            Ok(IosDevice {
                name: short_ios_name(&udid),
                id: udid,
            })
        })
        .collect()
}

/// 从 `{"deviceList":["a","b"]}` 里取出数组元素；不是这个形状就返回 None。
pub fn extract_device_list(output: &str) -> Option<Vec<String>> {
    let start = output.find("\"deviceList\"")?;
    let open = output[start..].find('[')? + start;
    let close = output[open..].find(']')? + open;
    Some(
        output[open + 1..close]
            .split(',')
            .map(|item| item.trim().trim_matches('"').to_owned())
            .filter(|item| !item.is_empty())
            .collect(),
    )
}

/// 界面上不显示完整 UDID：又长又不可读，且没必要把完整设备标识摆在屏幕上。
pub fn short_ios_name(udid: &str) -> String {
    let tail: String = udid
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("iPhone ····{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_go_ios_json_device_list() {
        let devices =
            parse_ios_devices(r#"{"deviceList":["REDACTEDSERIAL1","REDACTEDSERIAL2"]}"#).unwrap();

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "REDACTEDSERIAL1");
        assert_eq!(devices[0].name, "iPhone ····IAL1");
    }

    #[test]
    fn parses_plain_line_device_list() {
        let devices =
            parse_ios_devices("List of attached devices:\nREDACTEDSERIAL\nUDID: REDACTEDSERIAL2\n")
                .unwrap();

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[1].id, "REDACTEDSERIAL2");
    }

    #[test]
    fn empty_output_yields_no_devices() {
        assert!(parse_ios_devices("").unwrap().is_empty());
        assert!(parse_ios_devices(r#"{"deviceList":[]}"#)
            .unwrap()
            .is_empty());
    }

    /// 真机 1.2.1 在没接手机时的 `ios list` stdout 原样进解析器。
    /// 同一次运行的 stderr 有一条 WARN 日志行——它**不能**混进解析输入，
    /// 这条用例连同 [`fixture_stderr_is_not_parsed_as_devices`] 一起把这点钉住。
    #[test]
    fn fixture_empty_device_list_parses_to_nothing() {
        let fixture = include_str!("../tests/fixtures/go-ios-1.2.1/ios-list-empty.stdout.json");
        assert!(parse_ios_devices(fixture).unwrap().is_empty());
    }

    /// go-ios 的 stderr 是结构化 JSON 日志行。若实现把 stderr 也喂进解析器，
    /// 这条 WARN 会被当成一个「设备」——用例证明那样是错的。
    #[test]
    fn fixture_stderr_is_not_parsed_as_devices() {
        let stderr = include_str!("../tests/fixtures/go-ios-1.2.1/ios-list-empty.stderr.jsonl");
        // 日志行含空白，行式回退会判定为不可解析——正是我们要的「响亮失败」。
        assert!(parse_ios_devices(stderr).is_err());
    }

    #[test]
    fn rejects_unparsable_line() {
        assert!(parse_ios_devices("REDACTEDSERIAL with-space\n").is_err());
    }

    #[test]
    fn ignores_non_matching_json_shape() {
        assert!(extract_device_list(r#"{"other":[1]}"#).is_none());
    }

    #[test]
    fn short_name_keeps_only_the_tail() {
        assert_eq!(short_ios_name("REDACTEDSERIAL"), "iPhone ····RIAL");
        assert_eq!(short_ios_name("ab"), "iPhone ····ab");
    }
}
