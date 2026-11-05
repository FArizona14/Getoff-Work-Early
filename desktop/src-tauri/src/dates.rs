use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Weekday};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DateParse {
    pub planned_date: Option<String>,
    pub due_at: Option<i64>,
    pub date_kind: String,
    pub label: String,
    pub matched: String,
}

fn weekday(c: char) -> Option<Weekday> {
    Some(match c {
        '一' => Weekday::Mon,
        '二' => Weekday::Tue,
        '三' => Weekday::Wed,
        '四' => Weekday::Thu,
        '五' => Weekday::Fri,
        '六' => Weekday::Sat,
        '日' | '天' => Weekday::Sun,
        _ => return None,
    })
}
fn fmt(date: NaiveDate) -> String { date.format("%Y-%m-%d").to_string() }
fn exact(date: NaiveDate, matched: &str, due: bool) -> DateParse {
    let due_at = due.then(|| Local.from_local_datetime(&date.and_hms_opt(18, 0, 0).unwrap()).single().unwrap().timestamp());
    DateParse { planned_date: Some(fmt(date)), due_at, date_kind: "exact".into(), label: format!("{}：{} · 周{}", if due {"截止"} else {"计划"}, date.format("%m 月 %d 日"), "一二三四五六日".chars().nth(date.weekday().num_days_from_monday() as usize).unwrap()), matched: matched.into() }
}
fn next_weekday(today: NaiveDate, target: Weekday, next_week: bool, this_week: bool) -> NaiveDate {
    let current = today.weekday().num_days_from_monday() as i64;
    let wanted = target.num_days_from_monday() as i64;
    let days = if next_week { 7 - current + wanted } else if this_week { (wanted-current).max(0) } else { (wanted-current+7)%7 };
    today + Duration::days(days)
}
fn number_before(text: &str, marker: &str) -> Option<i64> {
    let pos=text.find(marker)?; let prefix=&text[..pos];
    let digits:String=prefix.chars().rev().take_while(|c|c.is_ascii_digit()).collect::<String>().chars().rev().collect();
    if !digits.is_empty(){return digits.parse().ok()}
    prefix.chars().last().and_then(|c| match c {'一'=>Some(1),'二'|'两'=>Some(2),'三'=>Some(3),'四'=>Some(4),'五'=>Some(5),'六'=>Some(6),'七'=>Some(7),'八'=>Some(8),'九'=>Some(9),_=>None})
}
pub fn parse_at(text: &str, today: NaiveDate) -> DateParse {
    let due = text.contains("截止") || text.contains("之前") || text.contains("前") || text.contains("月底");
    for (word, days) in [("今天",0),("明天",1),("后天",2)] { if text.contains(word) { return exact(today+Duration::days(days),word,due); } }
    if text.contains("月底") || text.contains("月末") { let first=if today.month()==12 {NaiveDate::from_ymd_opt(today.year()+1,1,1).unwrap()} else {NaiveDate::from_ymd_opt(today.year(),today.month()+1,1).unwrap()}; return exact(first-Duration::days(1),"月底前",true); }
    if let Some(days)=number_before(text,"天后").filter(|n|*n<=365) { return exact(today+Duration::days(days),&format!("{days}天后"),due); }
    for prefix in ["下周","下星期","本周","本星期","周","星期"] { if let Some(pos)=text.find(prefix) { if let Some(c)=text[pos+prefix.len()..].chars().next() { if let Some(w)=weekday(c) { let d=next_weekday(today,w,prefix.starts_with('下'),prefix.starts_with('本')); return exact(d,&format!("{prefix}{c}"),due); } } } }
    if let Some(pos)=text.find("下个月").or_else(||text.find("下月")) {
        let matched=if text[pos..].starts_with("下个月"){"下个月"}else{"下月"}; let (y,m)=if today.month()==12{(today.year()+1,1)}else{(today.year(),today.month()+1)};
        let rest=&text[pos+matched.len()..]; let digits:String=rest.chars().skip_while(|c|!c.is_ascii_digit()).take_while(|c|c.is_ascii_digit()).collect();
        if let Ok(day)=digits.parse::<u32>() { if let Some(d)=NaiveDate::from_ymd_opt(y,m,day){return exact(d,&format!("{matched}{day}号"),due)} }
        return DateParse { planned_date:Some(format!("{y:04}-{m:02}")),due_at:None,date_kind:"month_tbd".into(),label:format!("计划：{m} 月 · 日期待定"),matched:matched.into() };
    }
    DateParse { planned_date:None,due_at:None,date_kind:"none".into(),label:"未识别到日期，默认进入今日".into(),matched:String::new() }
}
pub fn parse(text: &str) -> DateParse { parse_at(text, Local::now().date_naive()) }

#[cfg(test)] mod tests { use super::*; #[test] fn common_chinese_dates(){ let d=NaiveDate::from_ymd_opt(2026,11,5).unwrap(); assert_eq!(parse_at("下周三提交报价",d).planned_date.as_deref(),Some("2026-11-11")); assert_eq!(parse_at("明天处理",d).planned_date.as_deref(),Some("2026-11-06")); assert_eq!(parse_at("下个月15号复盘",d).planned_date.as_deref(),Some("2026-12-15")); assert_eq!(parse_at("下个月复盘",d).date_kind,"month_tbd"); assert!(parse_at("月底前完成",d).due_at.is_some()); } }
