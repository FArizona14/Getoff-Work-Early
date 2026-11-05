use crate::repository::Store;
use chrono::{Datelike, Duration, Local, LocalResult, NaiveDate, TimeZone, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

fn db_error(error: rusqlite::Error) -> String {
    format!("数据库操作失败：{error}")
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportCandidate {
    pub id: i64,
    pub title: String,
    pub result: String,
    pub status: String,
    pub project: String,
    pub priority: String,
    pub planned_date: Option<String>,
    pub completed_at: Option<i64>,
    pub updated_at: i64,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportInput {
    pub report_type: String,
    pub range_start: String,
    pub range_end: String,
    pub template: String,
    pub source_ids: Vec<i64>,
    pub note: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEdit {
    pub id: i64,
    pub title: String,
    pub body: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSummary {
    pub id: i64,
    pub report_type: String,
    pub range_start: String,
    pub range_end: String,
    pub title: String,
    pub generated_by: String,
    pub generation_reason: String,
    pub source_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSource {
    pub id: i64,
    pub task_id: Option<i64>,
    pub title: String,
    pub result: String,
    pub status: String,
    pub project: String,
    pub planned_date: Option<String>,
    pub completed_at: Option<i64>,
    pub section: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportVersion {
    pub id: i64,
    pub version_kind: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDetail {
    pub id: i64,
    pub report_type: String,
    pub range_start: String,
    pub range_end: String,
    pub template: String,
    pub title: String,
    pub body: String,
    pub generated_by: String,
    pub generation_reason: String,
    pub source_cutoff: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub sources: Vec<ReportSource>,
    pub versions: Vec<ReportVersion>,
}

fn parse_day(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "报告日期格式无效".into())
}

fn local_timestamp(day: NaiveDate) -> Result<i64, String> {
    let time = day.and_hms_opt(0, 0, 0).ok_or("报告日期无效")?;
    match Local.from_local_datetime(&time) {
        LocalResult::Single(value) | LocalResult::Ambiguous(value, _) => Ok(value.timestamp()),
        LocalResult::None => Err("报告日期不在有效本地时间范围内".into()),
    }
}

fn range(input_type: &str, start: &str, end: &str) -> Result<(NaiveDate, NaiveDate, i64, i64), String> {
    if !["daily", "weekly"].contains(&input_type) {
        return Err("报告类型无效".into());
    }
    let start_day = parse_day(start)?;
    let end_day = parse_day(end)?;
    let days = (end_day - start_day).num_days();
    if !(0..=31).contains(&days) {
        return Err("报告范围需在 1–32 天内".into());
    }
    Ok((
        start_day,
        end_day,
        local_timestamp(start_day)?,
        local_timestamp(end_day + Duration::days(1))? - 1,
    ))
}

fn clean(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn status_label(value: &str) -> &'static str {
    match value {
        "completed" => "已完成",
        "in_progress" => "进行中",
        "cancelled" => "已取消",
        "inbox" => "待安排",
        _ => "待办",
    }
}

fn item(task: &ReportCandidate, template: &str) -> String {
    let title = clean(&task.title);
    let project = clean(&task.project);
    let result = clean(&task.result);
    match template {
        "simple" => format!("- {title}"),
        "results" => format!(
            "- {title}：{}",
            if result.is_empty() { "待补充结果 / 数据" } else { &result }
        ),
        _ => {
            let mut parts = Vec::new();
            if !project.is_empty() {
                parts.push(format!("项目：{project}"));
            }
            parts.push(format!("状态：{}", status_label(&task.status)));
            if !result.is_empty() {
                parts.push(format!("结果：{result}"));
            }
            format!("- {title}（{}）", parts.join("；"))
        }
    }
}

fn unique(tasks: impl IntoIterator<Item = ReportCandidate>) -> Vec<ReportCandidate> {
    let mut seen = HashSet::new();
    tasks
        .into_iter()
        .filter(|task| seen.insert(clean(&task.title).to_lowercase()))
        .collect()
}

fn section(title: &str, tasks: Vec<ReportCandidate>, template: &str, placeholder: &str) -> String {
    let tasks = unique(tasks);
    let content = if tasks.is_empty() {
        format!("- 待补充：{placeholder}")
    } else {
        tasks.iter().map(|task| item(task, template)).collect::<Vec<_>>().join("\n")
    };
    format!("## {title}\n\n{content}")
}

pub fn local_body(input: &ReportInput, tasks: &[ReportCandidate]) -> Result<String, String> {
    if !["simple", "detailed", "results"].contains(&input.template.as_str()) {
        return Err("报告模板无效".into());
    }
    let (_, end_day, _, _) = range(&input.report_type, &input.range_start, &input.range_end)?;
    let completed = tasks.iter().filter(|task| task.status == "completed").cloned().collect::<Vec<_>>();
    let outcomes = tasks.iter().filter(|task| !task.result.trim().is_empty()).cloned().collect::<Vec<_>>();
    let pending = tasks.iter().filter(|task| !["completed", "cancelled"].contains(&task.status.as_str())).cloned().collect::<Vec<_>>();
    let risks = tasks.iter().filter(|task| task.status == "cancelled" || (task.priority == "high" && task.status != "completed")).cloned().collect::<Vec<_>>();
    let learning = completed.iter().filter(|task| {
        let text = format!("{} {} {}", task.title, task.project, task.result).to_lowercase();
        ["学习", "培训", "课程", "复盘", "调研", "learn", "course", "research"].iter().any(|word| text.contains(word))
    }).cloned().collect::<Vec<_>>();
    let end_text = end_day.format("%Y-%m-%d").to_string();
    let next = pending.iter().filter(|task| task.planned_date.as_deref().is_some_and(|date| date > end_text.as_str())).cloned().chain(pending.clone()).collect::<Vec<_>>();
    let mut sections = if input.report_type == "daily" {
        vec![
            section("今日完成", completed, &input.template, "补充今天完成的工作。"),
            section("关键成果 / 数据", outcomes, &input.template, "补充可验证的结果、数据或影响。"),
            section("进行中事项", pending.clone(), &input.template, "补充仍在推进的事项。"),
            section("风险与阻塞", risks, &input.template, "如无风险可写“暂无”。"),
            section("明日计划", next, &input.template, "补充计划内容及预期目标。"),
        ]
    } else {
        vec![
            section("本周重点工作", completed, &input.template, "补充动作、关键细节和最终结果。"),
            section("主要成果与指标进展", outcomes, &input.template, "补充可验证指标，不能确认的数据请保留待补充。"),
            section("问题、风险与协作需求", risks, &input.template, "如无风险可写“暂无”，如需协作请说明对象与事项。"),
            section("个人学习", learning, &input.template, "补充学习模块及可验证的学习成果。"),
            section("下周计划", next, &input.template, "补充计划内容及预期目标。"),
        ]
    };
    if !input.note.trim().is_empty() {
        sections.push(format!("## 补充说明\n\n- {}", clean(&input.note)));
    }
    Ok(sections.join("\n\n"))
}

fn source_section(report_type: &str, task: &ReportCandidate, range_end: &str) -> &'static str {
    if task.planned_date.as_deref().is_some_and(|day| day > range_end) || !["completed", "cancelled"].contains(&task.status.as_str()) {
        return if report_type == "daily" { "明日计划" } else { "下周计划" };
    }
    if task.status == "cancelled" {
        return if report_type == "daily" { "风险与阻塞" } else { "问题、风险与协作需求" };
    }
    if !task.result.trim().is_empty() {
        return if report_type == "daily" { "关键成果 / 数据" } else { "主要成果与指标进展" };
    }
    if report_type == "daily" { "今日完成" } else { "本周重点工作" }
}

impl Store {
    pub fn report_candidates(&self, report_type: &str, start: &str, end: &str) -> Result<Vec<ReportCandidate>, String> {
        if self.is_locked() {
            return Err("应用已锁定，请先解锁".into());
        }
        let (_, end_day, start_at, end_at) = range(report_type, start, end)?;
        let next_end = (end_day + Duration::days(if report_type == "daily" { 1 } else { 7 })).format("%Y-%m-%d").to_string();
        let mut statement = self.db.prepare(
            "SELECT id,title,result,status,project,priority,planned_date,completed_at,updated_at FROM tasks
             WHERE (completed_at BETWEEN ?1 AND ?2) OR (updated_at BETWEEN ?1 AND ?2)
                OR (planned_date BETWEEN ?3 AND ?4)
             ORDER BY status='completed' DESC,coalesce(completed_at,updated_at) DESC,id DESC LIMIT 2000"
        ).map_err(db_error)?;
        let rows = statement.query_map(params![start_at, end_at, start, next_end], |row| Ok((
            row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?, row.get::<_, Option<Vec<u8>>>(2)?,
            row.get::<_, String>(3)?, row.get::<_, Option<Vec<u8>>>(4)?, row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?, row.get::<_, Option<i64>>(7)?, row.get::<_, i64>(8)?
        ))).map_err(db_error)?;
        let mut output = Vec::new();
        for row in rows {
            let (id, title, result, status, project, priority, planned_date, completed_at, updated_at) = row.map_err(db_error)?;
            output.push(ReportCandidate {
                id,
                title: self.crypto.decrypt(&title)?,
                result: result.map(|value| self.crypto.decrypt(&value)).transpose()?.unwrap_or_default(),
                status,
                project: project.map(|value| self.crypto.decrypt(&value)).transpose()?.unwrap_or_default(),
                priority,
                planned_date,
                completed_at,
                updated_at,
            });
        }
        Ok(output)
    }

    pub fn selected_report_candidates(&self, input: &ReportInput) -> Result<Vec<ReportCandidate>, String> {
        let selected = input.source_ids.iter().copied().collect::<HashSet<_>>();
        if selected.is_empty() {
            return Err("请至少选择一条来源记录".into());
        }
        let tasks = self.report_candidates(&input.report_type, &input.range_start, &input.range_end)?
            .into_iter().filter(|task| selected.contains(&task.id)).collect::<Vec<_>>();
        if tasks.len() != selected.len() {
            return Err("部分来源记录已变化，请重新读取后再生成".into());
        }
        Ok(tasks)
    }

    pub fn generate_local_report(&mut self, input: ReportInput) -> Result<ReportDetail, String> {
        let tasks = self.selected_report_candidates(&input)?;
        let body = local_body(&input, &tasks)?;
        self.create_report(input, tasks, body, "local", "manual")
    }

    pub fn create_report(&mut self, input: ReportInput, tasks: Vec<ReportCandidate>, body: String, generated_by: &str, reason: &str) -> Result<ReportDetail, String> {
        range(&input.report_type, &input.range_start, &input.range_end)?;
        if tasks.is_empty() || body.trim().is_empty() || body.chars().count() > 100_000 {
            return Err("报告内容或来源无效".into());
        }
        if !["local", "ai"].contains(&generated_by) || !["manual", "automatic", "catchup"].contains(&reason) {
            return Err("报告生成方式无效".into());
        }
        let now = Utc::now().timestamp();
        let title = format!("{} · {}—{}", if input.report_type == "daily" { "工作日报" } else { "工作周报" }, input.range_start, input.range_end);
        let title_encrypted = self.crypto.encrypt(&title)?;
        let body_encrypted = self.crypto.encrypt(body.trim())?;
        let sources = tasks.iter().map(|task| Ok((
            task.id,
            self.crypto.encrypt(&task.title)?,
            if task.result.trim().is_empty() { None } else { Some(self.crypto.encrypt(&task.result)?) },
            task.status.clone(),
            if task.project.trim().is_empty() { None } else { Some(self.crypto.encrypt(&task.project)?) },
            task.planned_date.clone(),
            task.completed_at,
            source_section(&input.report_type, task, &input.range_end).to_string(),
        ))).collect::<Result<Vec<_>, String>>()?;
        let transaction = self.db.transaction().map_err(db_error)?;
        transaction.execute(
            "INSERT INTO reports(report_type,range_start,range_end,template,title,auto_body,edited_body,generated_by,generation_reason,source_cutoff,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?6,?7,?8,?9,?9,?9)",
            params![input.report_type,input.range_start,input.range_end,input.template,title_encrypted,body_encrypted,generated_by,reason,now]
        ).map_err(db_error)?;
        let id = transaction.last_insert_rowid();
        transaction.execute("INSERT INTO report_versions(report_id,version_kind,body,created_at) VALUES(?1,'generated',?2,?3)", params![id,body_encrypted,now]).map_err(db_error)?;
        for (task_id, task_title, task_result, task_status, task_project, planned_date, completed_at, section) in sources {
            transaction.execute(
                "INSERT INTO report_sources(report_id,task_id,task_title,task_result,task_status,task_project,planned_date,completed_at,section,added_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![id,task_id,task_title,task_result,task_status,task_project,planned_date,completed_at,section,now]
            ).map_err(db_error)?;
        }
        transaction.commit().map_err(db_error)?;
        self.report(id)
    }

    pub fn reports(&self) -> Result<Vec<ReportSummary>, String> {
        if self.is_locked() { return Err("应用已锁定，请先解锁".into()); }
        let mut statement = self.db.prepare(
            "SELECT r.id,r.report_type,r.range_start,r.range_end,r.title,r.generated_by,r.generation_reason,
                    (SELECT count(*) FROM report_sources s WHERE s.report_id=r.id),r.created_at,r.updated_at
             FROM reports r ORDER BY r.updated_at DESC,r.id DESC LIMIT 300"
        ).map_err(db_error)?;
        let rows = statement.query_map([], |row| Ok((
            row.get::<_,i64>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,Vec<u8>>(4)?,
            row.get::<_,String>(5)?,row.get::<_,String>(6)?,row.get::<_,i64>(7)?,row.get::<_,i64>(8)?,row.get::<_,i64>(9)?
        ))).map_err(db_error)?;
        let mut output = Vec::new();
        for row in rows {
            let (id,report_type,range_start,range_end,title,generated_by,generation_reason,source_count,created_at,updated_at)=row.map_err(db_error)?;
            output.push(ReportSummary{id,report_type,range_start,range_end,title:self.crypto.decrypt(&title)?,generated_by,generation_reason,source_count,created_at,updated_at});
        }
        Ok(output)
    }

    pub fn report(&self, id: i64) -> Result<ReportDetail, String> {
        if self.is_locked() { return Err("应用已锁定，请先解锁".into()); }
        let row = self.db.query_row(
            "SELECT id,report_type,range_start,range_end,template,title,edited_body,generated_by,generation_reason,source_cutoff,created_at,updated_at FROM reports WHERE id=?1",
            [id], |row| Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,Vec<u8>>(5)?,row.get::<_,Vec<u8>>(6)?,row.get::<_,String>(7)?,row.get::<_,String>(8)?,row.get::<_,i64>(9)?,row.get::<_,i64>(10)?,row.get::<_,i64>(11)?))
        ).optional().map_err(db_error)?.ok_or("报告不存在")?;
        let mut sources_statement = self.db.prepare("SELECT id,task_id,task_title,task_result,task_status,task_project,planned_date,completed_at,section FROM report_sources WHERE report_id=?1 ORDER BY id").map_err(db_error)?;
        let source_rows = sources_statement.query_map([id], |source| Ok((source.get::<_,i64>(0)?,source.get::<_,Option<i64>>(1)?,source.get::<_,Vec<u8>>(2)?,source.get::<_,Option<Vec<u8>>>(3)?,source.get::<_,String>(4)?,source.get::<_,Option<Vec<u8>>>(5)?,source.get::<_,Option<String>>(6)?,source.get::<_,Option<i64>>(7)?,source.get::<_,String>(8)?))).map_err(db_error)?;
        let mut sources = Vec::new();
        for source in source_rows {
            let (source_id,task_id,title,result,status,project,planned_date,completed_at,section)=source.map_err(db_error)?;
            sources.push(ReportSource{id:source_id,task_id,title:self.crypto.decrypt(&title)?,result:result.map(|value|self.crypto.decrypt(&value)).transpose()?.unwrap_or_default(),status,project:project.map(|value|self.crypto.decrypt(&value)).transpose()?.unwrap_or_default(),planned_date,completed_at,section});
        }
        let mut versions_statement = self.db.prepare("SELECT id,version_kind,body,created_at FROM report_versions WHERE report_id=?1 ORDER BY created_at DESC,id DESC").map_err(db_error)?;
        let version_rows = versions_statement.query_map([id], |version| Ok((version.get::<_,i64>(0)?,version.get::<_,String>(1)?,version.get::<_,Vec<u8>>(2)?,version.get::<_,i64>(3)?))).map_err(db_error)?;
        let mut versions=Vec::new();
        for version in version_rows{let(version_id,version_kind,version_body,created_at)=version.map_err(db_error)?;versions.push(ReportVersion{id:version_id,version_kind,body:self.crypto.decrypt(&version_body)?,created_at});}
        Ok(ReportDetail{id:row.0,report_type:row.1,range_start:row.2,range_end:row.3,template:row.4,title:self.crypto.decrypt(&row.5)?,body:self.crypto.decrypt(&row.6)?,generated_by:row.7,generation_reason:row.8,source_cutoff:row.9,created_at:row.10,updated_at:row.11,sources,versions})
    }

    pub fn save_report(&mut self, input: ReportEdit) -> Result<ReportDetail, String> {
        let title = input.title.trim();
        let body = input.body.trim();
        if title.is_empty() || title.chars().count() > 200 || body.is_empty() || body.chars().count() > 100_000 {
            return Err("报告标题或正文长度无效".into());
        }
        let title = self.crypto.encrypt(title)?;
        let body = self.crypto.encrypt(body)?;
        let now = Utc::now().timestamp();
        let transaction = self.db.transaction().map_err(db_error)?;
        if transaction.execute("UPDATE reports SET title=?1,edited_body=?2,updated_at=?3 WHERE id=?4",params![title,body,now,input.id]).map_err(db_error)? == 0 {
            return Err("报告不存在".into());
        }
        transaction.execute("INSERT INTO report_versions(report_id,version_kind,body,created_at) VALUES(?1,'edited',?2,?3)",params![input.id,body,now]).map_err(db_error)?;
        transaction.commit().map_err(db_error)?;
        self.report(input.id)
    }

    pub fn sync_report(&mut self, id: i64) -> Result<ReportDetail, String> {
        let report = self.report(id)?;
        let existing = report.sources.iter().filter_map(|source| source.task_id).collect::<HashSet<_>>();
        let latest = self.report_candidates(&report.report_type, &report.range_start, &report.range_end)?.into_iter().filter(|task| !existing.contains(&task.id)).collect::<Vec<_>>();
        if latest.is_empty() { return Ok(report); }
        let now = Utc::now().timestamp();
        let append = format!("\n\n## 同步新增记录 · {}\n\n{}", Local::now().format("%m月%d日 %H:%M"), latest.iter().map(|task| item(task,"detailed")).collect::<Vec<_>>().join("\n"));
        let body = format!("{}{}", report.body.trim_end(), append);
        let body_encrypted = self.crypto.encrypt(&body)?;
        let sources = latest.iter().map(|task| Ok((task.id,self.crypto.encrypt(&task.title)?,if task.result.is_empty(){None}else{Some(self.crypto.encrypt(&task.result)?)},task.status.clone(),if task.project.is_empty(){None}else{Some(self.crypto.encrypt(&task.project)?)},task.planned_date.clone(),task.completed_at,source_section(&report.report_type,task,&report.range_end).to_string()))).collect::<Result<Vec<_>,String>>()?;
        let transaction = self.db.transaction().map_err(db_error)?;
        transaction.execute("INSERT INTO report_versions(report_id,version_kind,body,created_at) VALUES(?1,'synced',?2,?3)",params![id,body_encrypted,now]).map_err(db_error)?;
        transaction.execute("UPDATE reports SET edited_body=?1,source_cutoff=?2,updated_at=?2 WHERE id=?3",params![body_encrypted,now,id]).map_err(db_error)?;
        for (task_id,title,result,status,project,planned_date,completed_at,section) in sources {
            transaction.execute("INSERT OR IGNORE INTO report_sources(report_id,task_id,task_title,task_result,task_status,task_project,planned_date,completed_at,section,added_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![id,task_id,title,result,status,project,planned_date,completed_at,section,now]).map_err(db_error)?;
        }
        transaction.commit().map_err(db_error)?;
        self.report(id)
    }

    pub fn report_text(&self, id: i64, markdown: bool) -> Result<(String, String), String> {
        let report = self.report(id)?;
        let body = if markdown { format!("# {}\n\n{}\n", report.title, report.body) } else { format!("{}\n\n{}\n", report.title, report.body.replace("## ", "")) };
        Ok((report.title, body))
    }

    pub fn report_auto_enabled(&self) -> bool { self.setting("report_auto_enabled").map(|value|value=="true").unwrap_or(true) }
    pub fn report_weekday(&self) -> i64 { self.setting("report_weekday").and_then(|value|value.parse().ok()).filter(|value|(1..=7).contains(value)).unwrap_or(5) }
    pub fn save_report_schedule(&self, enabled: bool, weekday: i64) -> Result<(), String> {
        if !(1..=7).contains(&weekday) { return Err("周报结算日无效".into()); }
        self.set("report_auto_enabled",&enabled.to_string())?;
        self.set("report_weekday",&weekday.to_string())
    }

    fn automatic_exists(&self, start: &str, end: &str) -> Result<bool, String> {
        self.db.query_row("SELECT EXISTS(SELECT 1 FROM reports WHERE report_type='weekly' AND range_start=?1 AND range_end=?2 AND generation_reason IN ('automatic','catchup'))",params![start,end],|row|row.get(0)).map_err(db_error)
    }

    fn automatic_week(&mut self, start: NaiveDate, end: NaiveDate, reason: &str) -> Result<Option<String>, String> {
        let start_text=start.format("%Y-%m-%d").to_string();let end_text=end.format("%Y-%m-%d").to_string();
        if self.automatic_exists(&start_text,&end_text)? { return Ok(None); }
        let tasks=self.report_candidates("weekly",&start_text,&end_text)?;
        if tasks.is_empty(){return Ok(None)}
        let input=ReportInput{report_type:"weekly".into(),range_start:start_text,range_end:end_text,template:"detailed".into(),source_ids:tasks.iter().map(|task|task.id).collect(),note:String::new()};
        let body=local_body(&input,&tasks)?;
        let report=self.create_report(input,tasks,body,"local",reason)?;
        self.set("report_notice",&format!("{}|{}",report.id,report.title))?;
        Ok(Some(report.title))
    }

    pub fn maybe_auto_report(&mut self, now: i64) -> Result<Option<String>, String> {
        if self.is_locked() || !self.report_auto_enabled() { return Ok(None); }
        let local = Local.timestamp_opt(now,0).single().ok_or("当前时间无效")?;
        let today=local.date_naive();let monday=today-Duration::days(today.weekday().num_days_from_monday() as i64);let settlement=monday+Duration::days(self.report_weekday()-1);
        if today==settlement {
            let worked=self.db.query_row("SELECT started_at,ended_at,target_seconds FROM workdays WHERE day=?1",[today.format("%Y-%m-%d").to_string()],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,Option<i64>>(1)?,row.get::<_,i64>(2)?))).optional().map_err(db_error)?;
            if worked.is_some_and(|(start,end,target)|end.is_some()||now-start>=target) {
                if let Some(title)=self.automatic_week(monday,settlement,"automatic")?{return Ok(Some(title))}
            }
        }
        if today>=monday {
            let previous_monday=monday-Duration::days(7);let previous_end=previous_monday+Duration::days(self.report_weekday()-1);
            if let Some(title)=self.automatic_week(previous_monday,previous_end,"catchup")?{return Ok(Some(title))}
        }
        Ok(None)
    }

    pub fn take_report_notice(&self) -> Result<Option<String>, String> {
        let notice=self.setting("report_notice");
        if notice.is_some(){self.db.execute("DELETE FROM settings WHERE key='report_notice'",[]).map_err(db_error)?;}
        Ok(notice.and_then(|value|value.split_once('|').map(|(_,title)|format!("{title}已沉淀，可随时检查"))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_never_invents_results() {
        let input=ReportInput{report_type:"weekly".into(),range_start:"2026-11-02".into(),range_end:"2026-11-06".into(),template:"results".into(),source_ids:vec![1],note:String::new()};
        let task=ReportCandidate{id:1,title:"完成发布流程".into(),result:String::new(),status:"completed".into(),project:"桌面应用".into(),priority:"high".into(),planned_date:None,completed_at:Some(1),updated_at:1};
        let body=local_body(&input,&[task]).unwrap();
        assert!(body.contains("待补充结果 / 数据"));
        assert!(body.contains("待补充：补充学习模块"));
    }

    #[test]
    fn daily_has_five_sections() {
        let input=ReportInput{report_type:"daily".into(),range_start:"2026-11-06".into(),range_end:"2026-11-06".into(),template:"simple".into(),source_ids:vec![1],note:String::new()};
        let task=ReportCandidate{id:1,title:"整理反馈".into(),result:"已输出清单".into(),status:"completed".into(),project:String::new(),priority:"medium".into(),planned_date:None,completed_at:Some(1),updated_at:1};
        assert_eq!(local_body(&input,&[task]).unwrap().matches("## ").count(),5);
    }
}
