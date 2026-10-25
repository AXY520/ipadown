use ipadown_store as store;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

const MAX_CONCURRENT_DOWNLOADS: usize = 2;
const MAX_PENDING_DOWNLOADS: usize = 8;

#[derive(Clone, Serialize)]
pub(super) struct Job {
    id: String,
    bundle_id: String,
    app_name: Option<String>,
    version_id: Option<String>,
    platform: store::search::DevicePlatform,
    account: String,
    /// queued | downloading | done | error
    status: String,
    downloaded: u64,
    error: Option<String>,
    /// 前端据此弹出双重认证对话框，而不是解析错误文案。
    needs_2fa: bool,
    #[serde(skip_serializing)]
    created_at: u64,
}

impl Job {
    fn new(
        id: &str,
        bundle_id: &str,
        version_id: Option<&str>,
        platform: store::search::DevicePlatform,
        account: &str,
    ) -> Self {
        Job {
            id: id.to_string(),
            bundle_id: bundle_id.to_string(),
            app_name: None,
            version_id: version_id.map(String::from),
            platform,
            account: account.to_string(),
            status: "queued".into(),
            downloaded: 0,
            error: None,
            needs_2fa: false,
            created_at: unix_now(),
        }
    }

    fn record_failure(&mut self, error: &store::StoreError) {
        self.status = "error".into();
        self.needs_2fa = needs_two_factor(error);
        self.error = Some(if self.needs_2fa {
            "需要双重验证码，请完成验证后重试下载".into()
        } else {
            crate::friendly_msg(&error.to_string())
        });
    }
}

pub(super) fn needs_two_factor(error: &store::StoreError) -> bool {
    matches!(error, store::StoreError::CodeRequired)
}

#[derive(Clone)]
pub(super) struct Registry {
    jobs: Arc<Mutex<HashMap<String, Job>>>,
    permits: Arc<Semaphore>,
}

pub(super) fn new_registry() -> Registry {
    Registry {
        jobs: Arc::new(Mutex::new(HashMap::new())),
        permits: Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS)),
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// 锁中毒时数据仍完整，取回继续而不是连环 panic。
fn jobs_map(registry: &Registry) -> std::sync::MutexGuard<'_, HashMap<String, Job>> {
    registry
        .jobs
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(super) fn download_filename(
    bundle_id: &str,
    version_id: Option<&str>,
    platform: store::search::DevicePlatform,
) -> Result<String, &'static str> {
    if bundle_id.is_empty()
        || bundle_id.len() > 255
        || bundle_id.contains("..")
        || !bundle_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err("非法 bundle id");
    }
    if let Some(version) = version_id {
        if version.is_empty()
            || version.len() > 32
            || !version.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err("非法版本 ID");
        }
    }
    Ok(format!(
        "{}_{}{}.ipa",
        bundle_id,
        version_id.unwrap_or("latest"),
        platform.file_suffix(),
    ))
}

pub(super) fn list(reg: &Registry) -> Vec<Job> {
    let reg = jobs_map(reg);
    let mut jobs: Vec<Job> = reg.values().cloned().collect();
    jobs.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    jobs
}

/// 账号重新认证成功后，移除该账号此前因 2FA 失败的任务。
pub(super) fn clear_two_factor_failures(reg: &Registry, account: &str) -> usize {
    let mut jobs = jobs_map(reg);
    let before = jobs.len();
    jobs.retain(|_, job| {
        !(job.needs_2fa && job.status == "error" && job.account.eq_ignore_ascii_case(account))
    });
    before - jobs.len()
}

pub(super) fn is_file_active(reg: &Registry, filename: &str) -> bool {
    jobs_map(reg).values().any(|job| {
        matches!(job.status.as_str(), "queued" | "downloading")
            && download_filename(&job.bundle_id, job.version_id.as_deref(), job.platform).as_deref()
                == Ok(filename)
    })
}

fn prune_finished(reg: &Registry) {
    const MAX_FINISHED_JOBS: usize = 100;
    let mut reg = jobs_map(reg);
    let mut finished: Vec<(u64, String)> = reg
        .values()
        .filter(|job| matches!(job.status.as_str(), "done" | "error"))
        .map(|job| (job.created_at, job.id.clone()))
        .collect();
    if finished.len() <= MAX_FINISHED_JOBS {
        return;
    }
    finished.sort();
    let remove_count = finished.len() - MAX_FINISHED_JOBS;
    for (_, id) in finished.into_iter().take(remove_count) {
        reg.remove(&id);
    }
}

fn update(reg: &Registry, id: &str, f: impl FnOnce(&mut Job)) {
    if let Some(job) = jobs_map(reg).get_mut(id) {
        f(job);
    }
}

/// 在同一临界区内完成去重、容量检查和预留。
/// `Ok(None)` 表示相同任务已在队列中，`Ok(Some(id))` 表示创建了新任务。
fn reserve_job(
    reg: &Registry,
    bundle_id: &str,
    version_id: Option<&str>,
    platform: store::search::DevicePlatform,
    account_email: &str,
) -> Result<Option<String>, &'static str> {
    let mut jobs = jobs_map(reg);
    if jobs.values().any(|job| {
        job.bundle_id == bundle_id
            && job.version_id.as_deref() == version_id
            && job.platform == platform
            && matches!(job.status.as_str(), "queued" | "downloading")
    }) {
        return Ok(None);
    }
    let pending = jobs
        .values()
        .filter(|job| matches!(job.status.as_str(), "queued" | "downloading"))
        .count();
    if pending >= MAX_PENDING_DOWNLOADS {
        return Err("下载队列已满，请等待现有任务完成后重试");
    }
    let id = crate::random_id();
    jobs.insert(
        id.clone(),
        Job::new(&id, bundle_id, version_id, platform, account_email),
    );
    Ok(Some(id))
}

/// 启动后台下载任务（走 ipatool 子进程，含 SAP 签名），返回是否复用已有任务。
pub(super) fn spawn_download(
    reg: Registry,
    bundle_id: String,
    version_id: Option<String>,
    platform: store::search::DevicePlatform,
    account_email: String,
    ipatool: store::ipatool::Ipatool,
) -> Result<bool, &'static str> {
    let filename = download_filename(&bundle_id, version_id.as_deref(), platform)?;
    let Some(id) = reserve_job(
        &reg,
        &bundle_id,
        version_id.as_deref(),
        platform,
        &account_email,
    )?
    else {
        return Ok(true);
    };

    let task_id = id.clone();
    tokio::spawn(async move {
        match reg.permits.clone().acquire_owned().await {
            Ok(_permit) => {
                if let Err(e) = run_download(
                    &reg,
                    &task_id,
                    &bundle_id,
                    version_id.as_deref(),
                    platform,
                    filename,
                    ipatool,
                )
                .await
                {
                    store::log_error!(
                        "download",
                        "job failed id={task_id} bundle={bundle_id} version={} platform={} error={e}",
                        version_id.as_deref().unwrap_or("latest"),
                        platform.ipatool_value()
                    );
                    update(&reg, &task_id, |job| job.record_failure(&e));
                }
            }
            Err(_) => update(&reg, &task_id, |job| {
                job.record_failure(&store::StoreError::UnexpectedResponse(
                    "下载队列已停止".into(),
                ));
            }),
        }
        prune_finished(&reg);
    });
    Ok(false)
}

async fn run_download(
    reg: &Registry,
    id: &str,
    bundle_id: &str,
    version_id: Option<&str>,
    platform: store::search::DevicePlatform,
    filename: String,
    ipatool: store::ipatool::Ipatool,
) -> store::Result<()> {
    let started = std::time::Instant::now();
    store::log_info!(
        "download",
        "job started id={id} bundle={bundle_id} version={} platform={} file={filename}",
        version_id.unwrap_or("latest"),
        platform.ipatool_value()
    );
    // 入队前同步校验会话（auth info + 真实商店请求），失败/2FA 直接让任务失败，
    // 前端轮询 needs_2fa 后弹验证码对话框。
    ipatool.validate_session().await?;
    // 用公开搜索 API 补充应用名（多区依次尝试）
    for country in ["CN", "US", "JP"] {
        if let Ok(app) = store::search::lookup(bundle_id, country, platform).await {
            update(reg, id, |j| j.app_name = Some(app.name));
            break;
        }
    }

    update(reg, id, |j| j.status = "downloading".into());
    let output = store::config::ipas_dir()?.join(&filename);

    // 进度监视：ipatool 先下载到 <out>.tmp（支持断点续传），完成后才组装为最终 IPA
    let tmp_path = output.with_file_name(format!("{filename}.tmp"));
    let watch_output = output.clone();
    let reg_w = reg.clone();
    let id_w = id.to_string();
    let watcher = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let size = match tokio::fs::metadata(&tmp_path).await {
                Ok(meta) => meta.len(),
                Err(_) => tokio::fs::metadata(&watch_output)
                    .await
                    .map(|meta| meta.len())
                    .unwrap_or(0),
            };
            if size > 0 {
                update(&reg_w, &id_w, |j| j.downloaded = size);
            }
        }
    });
    let result = ipatool
        .download(bundle_id, version_id, &output, platform)
        .await;
    watcher.abort();
    result?;

    let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    update(reg, id, |j| {
        j.status = "done".into();
        j.downloaded = size;
    });
    store::log_info!(
        "download",
        "job done id={id} bundle={bundle_id} file={filename} size={size} elapsed={}s",
        started.elapsed().as_secs()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        clear_two_factor_failures, download_filename, list, new_registry, reserve_job, Job,
        MAX_PENDING_DOWNLOADS,
    };
    use ipadown_store::search::DevicePlatform;
    use ipadown_store::StoreError;

    #[test]
    fn download_filename_accepts_store_identifiers() {
        assert_eq!(
            download_filename("com.example.app", Some("123456789"), DevicePlatform::Iphone)
                .unwrap(),
            "com.example.app_123456789.ipa"
        );
        assert_eq!(
            download_filename("com.example.app", None, DevicePlatform::Ipad).unwrap(),
            "com.example.app_latest_ipad.ipa"
        );
        assert_eq!(
            download_filename("com.example.app", Some("123"), DevicePlatform::Appletv).unwrap(),
            "com.example.app_123_appletv.ipa"
        );
    }

    #[test]
    fn download_filename_rejects_path_traversal() {
        assert!(download_filename("../outside", None, DevicePlatform::Iphone).is_err());
        assert!(download_filename(
            "com.example.app",
            Some("../../outside"),
            DevicePlatform::Iphone
        )
        .is_err());
        assert!(download_filename("com/example/app", Some("123"), DevicePlatform::Iphone).is_err());
    }

    #[test]
    fn two_factor_failure_is_structured_for_frontend() {
        let mut job = Job::new(
            "job-1",
            "com.example.app",
            Some("123"),
            DevicePlatform::Iphone,
            "a@example.com",
        );

        job.record_failure(&StoreError::CodeRequired);

        assert_eq!(job.status, "error");
        assert!(job.needs_2fa);
        assert!(job.error.is_some());
    }

    #[test]
    fn serialized_job_hides_internal_sorting_state() {
        let job = Job::new(
            "job-1",
            "com.example.app",
            Some("123"),
            DevicePlatform::Iphone,
            "a@example.com",
        );
        let value = serde_json::to_value(job).unwrap();

        assert!(value.get("created_at").is_none());
        assert_eq!(value["status"], "queued");
    }

    #[test]
    fn successful_authentication_clears_matching_two_factor_failures() {
        let registry = new_registry();
        let mut job = Job::new(
            "job-1",
            "com.example.app",
            Some("123"),
            DevicePlatform::Iphone,
            "a@example.com",
        );
        job.record_failure(&StoreError::CodeRequired);
        let mut other_error = Job::new(
            "job-2",
            "com.example.other",
            None,
            DevicePlatform::Iphone,
            "a@example.com",
        );
        other_error.record_failure(&StoreError::AuthFailed("network".into()));
        let mut other_account = Job::new(
            "job-3",
            "com.example.third",
            None,
            DevicePlatform::Iphone,
            "b@example.com",
        );
        other_account.record_failure(&StoreError::CodeRequired);
        let mut jobs = registry.jobs.lock().unwrap();
        jobs.insert(job.id.clone(), job);
        jobs.insert(other_error.id.clone(), other_error);
        jobs.insert(other_account.id.clone(), other_account);
        drop(jobs);

        assert_eq!(clear_two_factor_failures(&registry, "a@example.com"), 1);
        let remaining = list(&registry);
        assert_eq!(remaining.len(), 2);
        assert!(remaining.iter().all(|job| job.id != "job-1"));
    }

    #[test]
    fn queue_deduplicates_and_enforces_pending_limit() {
        let registry = new_registry();
        assert!(reserve_job(
            &registry,
            "com.example.app",
            Some("1"),
            DevicePlatform::Iphone,
            "a@example.com"
        )
        .unwrap()
        .is_some());
        assert!(reserve_job(
            &registry,
            "com.example.app",
            Some("1"),
            DevicePlatform::Iphone,
            "a@example.com"
        )
        .unwrap()
        .is_none());

        for index in 1..MAX_PENDING_DOWNLOADS {
            assert!(reserve_job(
                &registry,
                &format!("com.example.app{index}"),
                None,
                DevicePlatform::Iphone,
                "a@example.com"
            )
            .unwrap()
            .is_some());
        }
        assert!(reserve_job(
            &registry,
            "com.example.overflow",
            None,
            DevicePlatform::Iphone,
            "a@example.com"
        )
        .is_err());
    }

    #[test]
    fn queue_keeps_same_version_for_different_platforms_separate() {
        let registry = new_registry();
        assert!(reserve_job(
            &registry,
            "com.example.app",
            Some("1"),
            DevicePlatform::Iphone,
            "a@example.com"
        )
        .unwrap()
        .is_some());
        assert!(reserve_job(
            &registry,
            "com.example.app",
            Some("1"),
            DevicePlatform::Ipad,
            "a@example.com"
        )
        .unwrap()
        .is_some());
    }
}
