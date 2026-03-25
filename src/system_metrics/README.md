# system_metrics 模块说明

本目录负责**系统资源指标采集与查询**（CPU/内存/网络/磁盘/TCP/进程等），目标是：

- 低干扰后台采样
- 实时窗口数据供前端展示
- 可选持久化到 SQLite
- 支持历史聚合查询

---

## 目录职责

- `mod.rs`
  - 模块入口与对外 API 转发
  - 保持“薄入口”，避免业务细节堆积
- `prelude.rs`
  - 子模块共享依赖导入（平台/并发/时间等）
- `state.rs`
  - 常量、全局状态、采样参数、窗口与队列配置
- `sampler.rs`
  - 采样循环与调度控制（start/stop、唤醒、订阅策略）
- `collect/`
  - 指标采集与 point 构建
  - `collect/linux.rs`：Linux `/proc` 系列采集
  - `collect/windows.rs`：Windows API/PDH 系列采集
  - `collect/mod.rs`：跨平台拼装与统一输出
- `writer.rs`
  - 持久化写入通道与批量 flush、retention 清理
- `realtime.rs`
  - 实时窗口缓冲、降采样、summary 计算
- `service.rs`
  - `get_system_metrics` 实时查询响应组装
- `query.rs`
  - 历史查询与聚合映射（DB -> `SystemMetricsPoint`）
- `types.rs`
  - 对外结构体与内部采样结构

---

## 运行流程（简化）

1. `start_system_sampler` 启动采样循环（`sampler.rs`）
2. 采样线程调用 `collect_one_point`（`collect/*`）
3. 最新点写入实时窗口（`realtime.rs`）
4. 若开启持久化，异步入队到 writer（`writer.rs`）
5. 前端读取：
   - 实时数据走 `get_system_metrics`（`service.rs`）
   - 历史数据走 `query_historical_system_metrics`（`query.rs`）

---

## 当前关键策略（代码约定）

- 采样间隔可配置，范围受 `MIN/MAX_SAMPLE_INTERVAL_SECS` 限制
- 空闲时降频（`IDLE_PAUSE_INTERVAL_SECS`）
- 实时窗口上限：`MAX_REALTIME_WINDOW_SECS = 2天`
- 图表点数上限：`MAX_CHART_POINTS = 1200`
- 持久化批量：
  - `DB_FLUSH_BATCH_SIZE = 800`
  - `DB_FLUSH_INTERVAL = 5s`
- retention：默认保留约 `360` 天，定时清理

---

## 维护建议

- 新增平台采集逻辑优先落在 `collect/<os>.rs`，不要回灌到 `mod.rs`。
- 采样调度问题优先看 `sampler.rs`，写入堆积优先看 `writer.rs`。
- 新增查询字段时同步改三处：
  1) `types.rs`（结构）
  2) `writer.rs`（入库字段）
  3) `query.rs`（聚合 SQL + 映射）
- 任何采样频率调优都建议同时观察：
  - CPU 占用
  - DB 写入延迟
  - 前端曲线平滑度（downsample 后）

---

## 与其他模块关系

- 并列：`metrics/`（请求级业务指标）
- 调用配置：`config`（采样间隔、持久化开关等）
- 存储复用：通过 `metrics::db_pool()` 进行 DB 持久化
