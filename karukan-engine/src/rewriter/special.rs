//! Google Mozc-compatible special conversion candidates.
//!
//! The era table and the date conversion behavior are derived from Mozc's
//! `src/rewriter/date_rewriter.cc`.  The bundled TSV files are copied from
//! Mozc.  See `THIRD_PARTY_LICENSES` for the BSD-3-Clause attribution.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{RewriteOutput, Rewriter};

const EMOTICON_TSV: &str = include_str!("../../data/emoticon.tsv");
const READING_CORRECTION_TSV: &str = include_str!("../../data/reading_correction.tsv");

// Generated mechanically from Mozc's `kEraData` in date_rewriter.cc.
const ERA_DATA: &[(i32, &str)] = &[
    (645, "大化"),
    (650, "白雉"),
    (686, "朱鳥"),
    (701, "大宝"),
    (704, "慶雲"),
    (708, "和銅"),
    (715, "霊亀"),
    (717, "養老"),
    (724, "神亀"),
    (729, "天平"),
    (749, "天平感宝"),
    (749, "天平勝宝"),
    (757, "天平宝字"),
    (765, "天平神護"),
    (767, "神護景雲"),
    (770, "宝亀"),
    (781, "天応"),
    (782, "延暦"),
    (806, "大同"),
    (810, "弘仁"),
    (824, "天長"),
    (834, "承和"),
    (848, "嘉祥"),
    (851, "仁寿"),
    (854, "斉衡"),
    (857, "天安"),
    (859, "貞観"),
    (877, "元慶"),
    (885, "仁和"),
    (889, "寛平"),
    (898, "昌泰"),
    (901, "延喜"),
    (923, "延長"),
    (931, "承平"),
    (938, "天慶"),
    (947, "天暦"),
    (957, "天徳"),
    (961, "応和"),
    (964, "康保"),
    (968, "安和"),
    (970, "天禄"),
    (973, "天延"),
    (976, "貞元"),
    (978, "天元"),
    (983, "永観"),
    (985, "寛和"),
    (987, "永延"),
    (989, "永祚"),
    (990, "正暦"),
    (995, "長徳"),
    (999, "長保"),
    (1004, "寛弘"),
    (1012, "長和"),
    (1017, "寛仁"),
    (1021, "治安"),
    (1024, "万寿"),
    (1028, "長元"),
    (1037, "長暦"),
    (1040, "長久"),
    (1044, "寛徳"),
    (1046, "永承"),
    (1053, "天喜"),
    (1058, "康平"),
    (1065, "治暦"),
    (1069, "延久"),
    (1074, "承保"),
    (1077, "承暦"),
    (1081, "永保"),
    (1084, "応徳"),
    (1087, "寛治"),
    (1094, "嘉保"),
    (1096, "永長"),
    (1097, "承徳"),
    (1099, "康和"),
    (1104, "長治"),
    (1106, "嘉承"),
    (1108, "天仁"),
    (1110, "天永"),
    (1113, "永久"),
    (1118, "元永"),
    (1120, "保安"),
    (1124, "天治"),
    (1126, "大治"),
    (1131, "天承"),
    (1132, "長承"),
    (1135, "保延"),
    (1141, "永治"),
    (1142, "康治"),
    (1144, "天養"),
    (1145, "久安"),
    (1151, "仁平"),
    (1154, "久寿"),
    (1156, "保元"),
    (1159, "平治"),
    (1160, "永暦"),
    (1161, "応保"),
    (1163, "長寛"),
    (1165, "永万"),
    (1166, "仁安"),
    (1169, "嘉応"),
    (1171, "承安"),
    (1175, "安元"),
    (1177, "治承"),
    (1181, "養和"),
    (1182, "寿永"),
    (1184, "元暦"),
    (1185, "文治"),
    (1190, "建久"),
    (1199, "正治"),
    (1201, "建仁"),
    (1204, "元久"),
    (1206, "建永"),
    (1207, "承元"),
    (1211, "建暦"),
    (1213, "建保"),
    (1219, "承久"),
    (1222, "貞応"),
    (1224, "元仁"),
    (1225, "嘉禄"),
    (1227, "安貞"),
    (1229, "寛喜"),
    (1232, "貞永"),
    (1233, "天福"),
    (1234, "文暦"),
    (1235, "嘉禎"),
    (1238, "暦仁"),
    (1239, "延応"),
    (1240, "仁治"),
    (1243, "寛元"),
    (1247, "宝治"),
    (1249, "建長"),
    (1256, "康元"),
    (1257, "正嘉"),
    (1259, "正元"),
    (1260, "文応"),
    (1261, "弘長"),
    (1264, "文永"),
    (1275, "建治"),
    (1278, "弘安"),
    (1288, "正応"),
    (1293, "永仁"),
    (1299, "正安"),
    (1302, "乾元"),
    (1303, "嘉元"),
    (1306, "徳治"),
    (1308, "延慶"),
    (1311, "応長"),
    (1312, "正和"),
    (1317, "文保"),
    (1319, "元応"),
    (1321, "元亨"),
    (1324, "正中"),
    (1326, "嘉暦"),
    (1329, "元徳"),
    (1331, "元弘"),
    (1334, "建武"),
    (1336, "延元"),
    (1340, "興国"),
    (1346, "正平"),
    (1370, "建徳"),
    (1372, "文中"),
    (1375, "天授"),
    (1381, "弘和"),
    (1384, "元中"),
    (1390, "明徳"),
    (1394, "応永"),
    (1428, "正長"),
    (1429, "永享"),
    (1441, "嘉吉"),
    (1444, "文安"),
    (1449, "宝徳"),
    (1452, "享徳"),
    (1455, "康正"),
    (1457, "長禄"),
    (1460, "寛正"),
    (1466, "文正"),
    (1467, "応仁"),
    (1469, "文明"),
    (1487, "長享"),
    (1489, "延徳"),
    (1492, "明応"),
    (1501, "文亀"),
    (1504, "永正"),
    (1521, "大永"),
    (1528, "享禄"),
    (1532, "天文"),
    (1555, "弘治"),
    (1558, "永禄"),
    (1570, "元亀"),
    (1573, "天正"),
    (1592, "文禄"),
    (1596, "慶長"),
    (1615, "元和"),
    (1624, "寛永"),
    (1644, "正保"),
    (1648, "慶安"),
    (1652, "承応"),
    (1655, "明暦"),
    (1658, "万治"),
    (1661, "寛文"),
    (1673, "延宝"),
    (1681, "天和"),
    (1684, "貞享"),
    (1688, "元禄"),
    (1704, "宝永"),
    (1711, "正徳"),
    (1716, "享保"),
    (1736, "元文"),
    (1741, "寛保"),
    (1744, "延享"),
    (1748, "寛延"),
    (1751, "宝暦"),
    (1764, "明和"),
    (1772, "安永"),
    (1781, "天明"),
    (1789, "寛政"),
    (1801, "享和"),
    (1804, "文化"),
    (1818, "文政"),
    (1830, "天保"),
    (1844, "弘化"),
    (1848, "嘉永"),
    (1854, "安政"),
    (1860, "万延"),
    (1861, "文久"),
    (1864, "元治"),
    (1865, "慶応"),
    (1868, "明治"),
    (1912, "大正"),
    (1926, "昭和"),
    (1989, "平成"),
    (2019, "令和"),
];

struct EmoticonIndex {
    by_reading: HashMap<String, Vec<String>>,
    all: Vec<String>,
}

static EMOTICONS: OnceLock<EmoticonIndex> = OnceLock::new();
static READING_CORRECTIONS: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

/// A calendar date and clock time in the operating system's local time zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDateTime {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    weekday: u8,
}

impl LocalDateTime {
    fn local_now() -> Option<Self> {
        let seconds = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        let seconds: libc::time_t = seconds.try_into().ok()?;
        let mut local: libc::tm = unsafe { std::mem::zeroed() };
        // SAFETY: `seconds` and `local` are valid pointers for `localtime_r`,
        // and `local` remains alive until its fields have been copied below.
        let converted = unsafe { libc::localtime_r(&seconds, &mut local) };
        (!converted.is_null()).then_some(Self {
            year: local.tm_year + 1900,
            month: (local.tm_mon + 1) as u32,
            day: local.tm_mday as u32,
            hour: local.tm_hour as u32,
            minute: local.tm_min as u32,
            weekday: local.tm_wday as u8,
        })
    }
}

/// Parses an arithmetic expression with standard precedence and right-associative powers.
pub struct ExpressionParser {
    input: Vec<char>,
    position: usize,
}

impl ExpressionParser {
    fn evaluate(input: &str) -> Option<f64> {
        let mut parser = Self {
            input: input.chars().collect(),
            position: 0,
        };
        let value = parser.expression()?;
        (parser.position == parser.input.len() && value.is_finite()).then_some(value)
    }

    fn expression(&mut self) -> Option<f64> {
        let mut value = self.product()?;
        while let Some(operator) = self.peek() {
            match operator {
                '+' => {
                    self.position += 1;
                    value += self.product()?;
                }
                '-' => {
                    self.position += 1;
                    value -= self.product()?;
                }
                _ => break,
            }
            if !value.is_finite() {
                return None;
            }
        }
        Some(value)
    }

    fn product(&mut self) -> Option<f64> {
        let mut value = self.unary()?;
        while let Some(operator) = self.peek() {
            match operator {
                '*' => {
                    self.position += 1;
                    value *= self.unary()?;
                }
                '/' => {
                    self.position += 1;
                    let divisor = self.unary()?;
                    if divisor == 0.0 {
                        return None;
                    }
                    value /= divisor;
                }
                _ => break,
            }
            if !value.is_finite() {
                return None;
            }
        }
        Some(value)
    }

    fn unary(&mut self) -> Option<f64> {
        match self.peek() {
            Some('+') => {
                self.position += 1;
                self.unary()
            }
            Some('-') => {
                self.position += 1;
                Some(-self.unary()?)
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Option<f64> {
        let value = self.primary()?;
        if self.peek() == Some('^') {
            self.position += 1;
            let result = value.powf(self.unary()?);
            return result.is_finite().then_some(result);
        }
        Some(value)
    }

    fn primary(&mut self) -> Option<f64> {
        if self.peek() == Some('(') {
            self.position += 1;
            let value = self.expression()?;
            (self.next() == Some(')')).then_some(value)
        } else {
            self.number()
        }
    }

    fn number(&mut self) -> Option<f64> {
        let start = self.position;
        let mut decimal_point = false;
        while let Some(character) = self.peek() {
            if character.is_ascii_digit() {
                self.position += 1;
            } else if character == '.' && !decimal_point {
                decimal_point = true;
                self.position += 1;
            } else {
                break;
            }
        }
        (self.position > start)
            .then(|| {
                self.input[start..self.position]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .ok()
            })
            .flatten()
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    fn next(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.position += 1;
        Some(character)
    }
}

/// Rewriter for dates, times, arithmetic, Unicode scalar values, and bundled data tables.
#[derive(Default)]
pub struct SpecialConversionRewriter;

impl SpecialConversionRewriter {
    pub fn new() -> Self {
        Self
    }

    /// Generates candidates using the supplied local time, enabling boundary tests.
    pub fn rewrite_with_local_datetime(
        &self,
        candidate: &str,
        local_datetime: LocalDateTime,
    ) -> Vec<RewriteOutput> {
        let mut output = Vec::new();
        let mut seen = HashSet::new();
        append_unique(
            &mut output,
            &mut seen,
            date_candidates(candidate, local_datetime),
            "日付",
        );
        append_unique(
            &mut output,
            &mut seen,
            four_digit_candidates(candidate),
            "日付・時刻",
        );
        append_unique(
            &mut output,
            &mut seen,
            ad_year_candidates(candidate),
            "和暦",
        );
        append_unique(
            &mut output,
            &mut seen,
            calculation_candidate(candidate),
            "計算結果",
        );
        append_unique(
            &mut output,
            &mut seen,
            unicode_candidate(candidate),
            "Unicode",
        );
        append_unique(
            &mut output,
            &mut seen,
            version_candidate(candidate),
            "バージョン",
        );
        append_unique(
            &mut output,
            &mut seen,
            emoticon_candidates(candidate),
            "顔文字",
        );
        append_unique(
            &mut output,
            &mut seen,
            lookup(
                &READING_CORRECTIONS,
                READING_CORRECTION_TSV,
                candidate,
                1,
                0,
            ),
            "読み訂正",
        );
        output
    }
}

impl Rewriter for SpecialConversionRewriter {
    fn name(&self) -> &'static str {
        "special_conversion"
    }

    fn rewrite(&self, candidate: &str) -> Vec<RewriteOutput> {
        match LocalDateTime::local_now() {
            Some(local_datetime) => self.rewrite_with_local_datetime(candidate, local_datetime),
            None => {
                let fallback = LocalDateTime {
                    year: 1,
                    month: 1,
                    day: 1,
                    hour: 0,
                    minute: 0,
                    weekday: 1,
                };
                let mut output = self.rewrite_with_local_datetime(candidate, fallback);
                output.retain(|(_, description)| description.as_deref() != Some("日付"));
                output
            }
        }
    }
}

fn append_unique(
    output: &mut Vec<RewriteOutput>,
    seen: &mut HashSet<String>,
    values: Vec<String>,
    description: &str,
) {
    for value in values {
        if seen.insert(value.clone()) {
            output.push((value, Some(description.to_string())));
        }
    }
}

fn lookup(
    table: &OnceLock<HashMap<String, Vec<String>>>,
    data: &str,
    candidate: &str,
    key_column: usize,
    value_column: usize,
) -> Vec<String> {
    table
        .get_or_init(|| {
            let mut entries: HashMap<String, Vec<String>> = HashMap::new();
            for line in data.lines().filter(|line| !line.starts_with('#')) {
                let columns: Vec<_> = line.split('\t').collect();
                let (Some(key), Some(values)) =
                    (columns.get(key_column), columns.get(value_column))
                else {
                    continue;
                };
                for key in key.split(' ') {
                    if key.is_empty() {
                        continue;
                    }

                    let bucket = entries.entry(key.to_string()).or_default();
                    if !bucket.iter().any(|existing| existing == values) {
                        bucket.push(values.to_string());
                    }
                }
            }
            entries
        })
        .get(candidate)
        .cloned()
        .unwrap_or_default()
}

fn emoticon_candidates(candidate: &str) -> Vec<String> {
    let index = EMOTICONS.get_or_init(|| {
        let mut by_reading: HashMap<String, Vec<String>> = HashMap::new();
        let mut all = Vec::new();
        for line in EMOTICON_TSV.lines().filter(|line| !line.starts_with('\t')) {
            let columns: Vec<_> = line.split('\t').collect();
            let (Some(emoticon), Some(readings)) = (columns.first(), columns.get(1)) else {
                continue;
            };
            if !all.iter().any(|existing| existing == emoticon) {
                all.push((*emoticon).to_string());
            }
            for reading in readings.split(' ') {
                if reading.is_empty() {
                    continue;
                }
                let bucket = by_reading.entry(reading.to_string()).or_default();
                if !bucket.iter().any(|existing| existing == emoticon) {
                    bucket.push((*emoticon).to_string());
                }
            }
        }
        EmoticonIndex { by_reading, all }
    });
    if candidate == "かおもじ" {
        index.all.clone()
    } else {
        index.by_reading.get(candidate).cloned().unwrap_or_default()
    }
}

fn date_candidates(candidate: &str, local: LocalDateTime) -> Vec<String> {
    let relative_day = match candidate {
        "きょう" => Some(0),
        "あした" | "あす" => Some(1),
        "きのう" | "さくじつ" => Some(-1),
        "おととい" | "おとつい" | "いっさくじつ" => Some(-2),
        "さきおととい" => Some(-3),
        "あさって" | "みょうごにち" => Some(2),
        "しあさって" => Some(3),
        _ => None,
    };
    if let Some(diff) = relative_day {
        let (year, month, day) = shifted_date(local.year, local.month, local.day, diff);
        return date_formats(year, month, day);
    }

    let weekday = match candidate {
        "にちようび" | "にちよう" => Some(0),
        "げつようび" | "げつよう" => Some(1),
        "かようび" | "かよう" => Some(2),
        "すいようび" | "すいよう" => Some(3),
        "もくようび" | "もくよう" => Some(4),
        "きんようび" | "きんよう" => Some(5),
        "どようび" | "どよう" => Some(6),
        _ => None,
    };
    if let Some(target) = weekday {
        let diff = (target - i32::from(local.weekday)).rem_euclid(7);
        let (year, month, day) = shifted_date(local.year, local.month, local.day, diff);
        return date_formats(year, month, day);
    }

    let year_diff = match candidate {
        "ことし" => Some(0),
        "らいねん" => Some(1),
        "さくねん" | "きょねん" => Some(-1),
        "おととし" => Some(-2),
        "さらいねん" => Some(2),
        _ => None,
    };
    if let Some(diff) = year_diff {
        let year = local.year + diff;
        let mut output = vec![year.to_string(), format!("{year}年")];
        for era in era_candidates(year, 0) {
            output.push(format!("{era}年"));
        }
        return output;
    }

    let month_diff = match candidate {
        "こんげつ" => Some(0),
        "らいげつ" => Some(1),
        "せんげつ" => Some(-1),
        "せんせんげつ" => Some(-2),
        "さらいげつ" => Some(2),
        _ => None,
    };
    if let Some(diff) = month_diff {
        let (_, month) = shifted_month(local.year, local.month, diff);
        return vec![month.to_string(), format!("{month}月")];
    }

    match candidate {
        "いま" | "じこく" => time_formats(local.hour, local.minute),
        "にちじ" | "なう" => vec![format!(
            "{:04}/{:02}/{:02} {:02}:{:02}",
            local.year, local.month, local.day, local.hour, local.minute
        )],
        _ => Vec::new(),
    }
}

fn date_formats(year: i32, month: u32, day: u32) -> Vec<String> {
    let mut output = vec![
        format!("{year:04}/{month:02}/{day:02}"),
        format!("{year:04}-{month:02}-{day:02}"),
        format!("{year}年{month}月{day}日"),
    ];
    if let Some(era) = era_candidates(year, month).into_iter().next() {
        output.push(format!("{era}年{month}月{day}日"));
    }
    let weekday = weekday_name(weekday_for(year, month, day));
    output.push(format!("{weekday}曜日"));
    output
}

fn four_digit_candidates(candidate: &str) -> Vec<String> {
    let digits = normalize_digits(candidate);
    if digits.len() != 4 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Vec::new();
    }
    let first = digits[..2].parse::<u32>().ok();
    let second = digits[2..].parse::<u32>().ok();
    let (Some(first), Some(second)) = (first, second) else {
        return Vec::new();
    };
    let mut output = Vec::new();
    if (1..=12).contains(&first) && (1..=days_in_month(2000, first)).contains(&second) {
        output.push(format!("{first}月{second}日"));
        output.push(format!("{first:02}/{second:02}"));
    }
    output.extend(time_formats(first, second));
    output
}

fn ad_year_candidates(candidate: &str) -> Vec<String> {
    let Some(year) = candidate
        .strip_suffix("ねん")
        .or_else(|| candidate.strip_suffix('年'))
        .and_then(|year| normalize_digits(year).parse::<i32>().ok())
    else {
        return Vec::new();
    };
    era_candidates(year, 0)
        .into_iter()
        .map(|era| format!("{era}年"))
        .collect()
}

fn time_formats(hour: u32, minute: u32) -> Vec<String> {
    if hour >= 30 || minute >= 60 {
        return Vec::new();
    }
    let mut output = vec![
        format!("{hour:02}:{minute:02}"),
        format!("{hour}時{minute:02}分"),
    ];
    if minute == 30 {
        output.push(format!("{hour}時半"));
    }
    let (prefix, display_hour) = if (hour % 24) * 60 + minute < 12 * 60 {
        ("午前", hour % 24)
    } else {
        ("午後", (hour - 12) % 24)
    };
    output.push(format!("{prefix}{display_hour}時{minute}分"));
    if minute == 30 {
        output.push(format!("{prefix}{display_hour}時半"));
    }
    output
}

fn calculation_candidate(candidate: &str) -> Vec<String> {
    let Some(expression) = candidate.strip_suffix(['=', '＝']) else {
        return Vec::new();
    };
    let normalized: String = expression
        .chars()
        .map(|character| match character {
            '０'..='９' => char::from_u32(character as u32 - '０' as u32 + '0' as u32).unwrap(),
            '．' => '.',
            '＋' => '+',
            '－' | 'ー' => '-',
            '＊' => '*',
            '／' | '・' => '/',
            '＾' => '^',
            '（' => '(',
            '）' => ')',
            other => other,
        })
        .collect();
    ExpressionParser::evaluate(&normalized)
        .map(format_number)
        .into_iter()
        .collect()
}

fn unicode_candidate(candidate: &str) -> Vec<String> {
    let Some(hex) = candidate
        .strip_prefix("U+")
        .or_else(|| candidate.strip_prefix("u+"))
    else {
        return Vec::new();
    };
    if !(1..=6).contains(&hex.len()) || !hex.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Vec::new();
    }
    u32::from_str_radix(hex, 16)
        .ok()
        .and_then(char::from_u32)
        .map(|character| character.to_string())
        .into_iter()
        .collect()
}

fn version_candidate(candidate: &str) -> Vec<String> {
    (candidate == "ばーじょん")
        .then(|| format!("Karukan {}", env!("CARGO_PKG_VERSION")))
        .into_iter()
        .collect()
}

fn normalize_digits(candidate: &str) -> String {
    candidate
        .chars()
        .map(|character| match character {
            '０'..='９' => char::from_u32(character as u32 - '０' as u32 + '0' as u32).unwrap(),
            other => other,
        })
        .collect()
}

fn format_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_string()
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn era_candidates(year: i32, month: u32) -> Vec<String> {
    if !(645..=2200).contains(&year) {
        return Vec::new();
    }
    if year == 2019 {
        return match month {
            0 => vec!["令和元".to_string(), "平成31".to_string()],
            1..=4 => vec!["平成31".to_string()],
            _ => vec!["令和元".to_string()],
        };
    }

    for index in (0..ERA_DATA.len()).rev() {
        let (start, era) = ERA_DATA[index];
        if index == ERA_DATA.len() - 1 && year > start {
            return vec![format_era(era, year - start + 1)];
        }
        if index > 0 && ERA_DATA[index - 1].0 < year && year <= start {
            let mut output = Vec::new();
            if year == start {
                output.push(format_era(era, 1));
            }
            let (previous_start, previous_era) = ERA_DATA[index - 1];
            output.push(format_era(previous_era, year - previous_start + 1));
            return output;
        }
        if index == 0 && start <= year {
            return vec![format_era(era, year - start + 1)];
        }
    }
    Vec::new()
}

fn format_era(era: &str, era_year: i32) -> String {
    if era_year == 1 {
        format!("{era}元")
    } else {
        format!("{era}{era_year}")
    }
}

fn shifted_month(year: i32, month: u32, diff: i32) -> (i32, u32) {
    let total = year * 12 + month as i32 - 1 + diff;
    (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32)
}

fn shifted_date(year: i32, month: u32, day: u32, diff: i32) -> (i32, u32, u32) {
    civil_from_days(days_from_civil(year, month, day) + i64::from(diff))
}

fn weekday_for(year: i32, month: u32, day: u32) -> u8 {
    (days_from_civil(year, month, day) + 4).rem_euclid(7) as u8
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = i64::from(year - i32::from(month <= 2));
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + i64::from(day) - 1;
    era * 146_097 + year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year - 719_468
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        _ => 0,
    }
}

fn weekday_name(weekday: u8) -> &'static str {
    ["日", "月", "火", "水", "木", "金", "土"][weekday as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewriter::test_util::texts;

    fn local(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> LocalDateTime {
        LocalDateTime {
            year,
            month,
            day,
            hour,
            minute,
            weekday: weekday_for(year, month, day),
        }
    }

    fn rewrite(candidate: &str, local_datetime: LocalDateTime) -> Vec<String> {
        texts(
            &SpecialConversionRewriter::new()
                .rewrite_with_local_datetime(candidate, local_datetime),
        )
    }

    #[test]
    fn relative_dates_cross_days_months_and_leap_days() {
        let leap_day = local(2024, 2, 29, 23, 30);
        assert!(rewrite("あした", leap_day).contains(&"2024/03/01".to_string()));
        assert!(rewrite("きのう", leap_day).contains(&"2024/02/28".to_string()));
        assert!(rewrite("あした", local(2023, 12, 31, 0, 0)).contains(&"2024/01/01".to_string()));
    }

    #[test]
    fn weekdays_years_months_and_current_datetime_are_generated() {
        let value = local(2024, 2, 29, 9, 30);
        assert!(rewrite("げつようび", value).contains(&"2024/03/04".to_string()));
        assert!(rewrite("らいねん", value).contains(&"2025年".to_string()));
        assert!(rewrite("らいげつ", value).contains(&"3月".to_string()));
        assert!(rewrite("いま", value).contains(&"09:30".to_string()));
        assert_eq!(rewrite("にちじ", value), vec!["2024/02/29 09:30"]);
    }

    #[test]
    fn era_table_covers_taika_and_reiwa_boundaries() {
        assert_eq!(
            rewrite("645ねん", local(2024, 1, 1, 0, 0)),
            vec!["大化元年"]
        );
        assert_eq!(
            rewrite("1989ねん", local(2024, 1, 1, 0, 0)),
            vec!["平成元年", "昭和64年"]
        );
        assert_eq!(
            rewrite("2019ねん", local(2024, 1, 1, 0, 0)),
            vec!["令和元年", "平成31年"]
        );
        assert!(rewrite("2201ねん", local(2024, 1, 1, 0, 0)).is_empty());
        assert_eq!(era_candidates(645, 1), vec!["大化元"]);
        assert_eq!(era_candidates(2019, 4), vec!["平成31"]);
        assert_eq!(era_candidates(2019, 5), vec!["令和元"]);
    }

    #[test]
    fn four_digit_dates_and_times_validate_boundaries() {
        let result = rewrite("0130", local(2024, 1, 1, 0, 0));
        assert!(result.contains(&"1月30日".to_string()));
        assert!(result.contains(&"01:30".to_string()));
        assert!(result.contains(&"1時半".to_string()));
        assert!(rewrite("0230", local(2024, 1, 1, 0, 0)).contains(&"02:30".to_string()));
        assert!(rewrite("2430", local(2024, 1, 1, 0, 0)).contains(&"24時半".to_string()));
        assert!(rewrite("2959", local(2024, 1, 1, 0, 0)).contains(&"29:59".to_string()));
        assert!(rewrite("3000", local(2024, 1, 1, 0, 0)).is_empty());
        assert!(rewrite("2460", local(2024, 1, 1, 0, 0)).is_empty());
        assert!(rewrite("😀", local(2024, 1, 1, 0, 0)).is_empty());
    }

    #[test]
    fn calculations_honor_precedence_right_association_and_invalid_inputs() {
        assert_eq!(rewrite("1+2*3=", local(2024, 1, 1, 0, 0)), vec!["7"]);
        assert_eq!(rewrite("2^3^2=", local(2024, 1, 1, 0, 0)), vec!["512"]);
        assert_eq!(rewrite("(1+2)*3＝", local(2024, 1, 1, 0, 0)), vec!["9"]);
        assert_eq!(rewrite("１ー２＝", local(2024, 1, 1, 0, 0)), vec!["-1"]);
        assert_eq!(rewrite("1・2＝", local(2024, 1, 1, 0, 0)), vec!["0.5"]);
        for input in ["1/0=", "1+=", "1..2=", "1e3=", "1e309="] {
            assert!(
                rewrite(input, local(2024, 1, 1, 0, 0)).is_empty(),
                "{input}"
            );
        }
    }

    #[test]
    fn unicode_version_emoticon_and_reading_correction_are_generated() {
        assert_eq!(rewrite("U+611B", local(2024, 1, 1, 0, 0)), vec!["愛"]);
        assert_eq!(rewrite("u+10FFFF", local(2024, 1, 1, 0, 0)), vec!["􏿿"]);
        for input in ["U+D800", "U+110000", "U+", "U+1234567"] {
            assert!(
                rewrite(input, local(2024, 1, 1, 0, 0)).is_empty(),
                "{input}"
            );
        }
        assert!(
            rewrite("ばーじょん", local(2024, 1, 1, 0, 0)).contains(&"Karukan 0.1.0".to_string())
        );
        assert!(rewrite("かおもじ", local(2024, 1, 1, 0, 0)).contains(&"＼(^o^)／".to_string()));
        assert!(rewrite("にこにこ", local(2024, 1, 1, 0, 0)).contains(&"＼(^o^)／".to_string()));
        assert!(rewrite("あぼがど", local(2024, 1, 1, 0, 0)).contains(&"アボカド".to_string()));
        assert!(
            rewrite("しゅみれーしょん", local(2024, 1, 1, 0, 0))
                .contains(&"シミュレーション".to_string())
        );
    }
}
