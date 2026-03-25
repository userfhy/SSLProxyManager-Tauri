# metrics 模块说明

本目录负责**请求级指标与日志存储/查询**，主要目标：

- 低开销接收代理请求日志
- 批量写入 SQLite
- 提供实时聚合 + 历史查询能力
- 提供 dashboard/top/黑名单 等上层数据接口

> 说明：`system_metrics`（CPU/内存/磁盘等）在 `src/system_metrics/`，不在本目录。

---

## 目录职责

- `mod.rs`
  - 模块总入口、全局状态、缓存、聚合结构与对外 API
  - 装配 db/query/writer/helpers/models
- `db.rs`
  - DB 初始化、连接池、建表/索引、空间回收相关
- `writer.rs`
  - 写入通道与批量 flush（高吞吐核心）
- `query.rs`
  - 历史查询与聚合查询逻辑
- `helpers.rs`
  - top/path/upstream 归一化等辅助逻辑
- `models.rs`
  - 请求/聚合/查询响应结构定义

---

## 数据流（简化）

1. proxy 请求完成后生成 `RequestLogInsert`
2. writer 通道接收并缓冲
3. 到达批量阈值或时间窗口后 flush 入库
4. 同步更新实时内存聚合（短窗口）
5. dashboard/query API 从：
   - 实时缓存（低延迟）
   - 或 SQLite 历史数据（长窗口）
   读取并组装响应

---

## 当前关键策略（从代码约定抽取）

- 批量写入：
  - `DB_FLUSH_BATCH_SIZE = 2000`
  - `DB_FLUSH_INTERVAL = 5s`
- 留存策略：
  - 请求日志保留约 `730` 天
  - 定期执行 retention 清理
- 空间回收：
  - 受最小删除行数、freelist 页面数与冷却时间约束，避免频繁 VACUUM
- 指标缓存：
  - `METRICS_CACHE_TTL = 500ms`，平衡实时性和查询成本

---

## 维护建议

- 新增查询优先放 `query.rs`，`mod.rs` 只保留入口与状态编排。
- 新增持久化行为优先放 `writer.rs`/`db.rs`，不要把 SQL 零散写在 `mod.rs`。
- 任何影响吞吐的改动都建议观察：
  1) flush 批量大小
  2) flush 间隔
  3) SQL 语句复杂度
  4) 索引命中情况
- 结构体字段调整时，先同步 `models.rs`，再补齐 query/writer 映射。

---

## 与其他模块关系

- 上游调用方：`proxy`（请求日志来源）
- 并列模块：`system_metrics`（系统资源指标）
- 配置来源：`config`（存储开关、行为参数）
