#include "backend.h"
#include "rust_bridge.h"

#include "aibox-ffi/src/incus.rs.h"

void Backend::loadDemoData() {
    const auto state = aibox::ffi::build_demo_state_ffi(
        m_recordSessions, aibox::ffi::toRust(m_osStoragePath));

    m_environments = aibox::ffi::fromRustJson(state.environments_json).toList();
    m_count = m_environments.size();
    m_operationHistory = aibox::ffi::fromRustJson(state.operation_history_json).toList();

    if (m_secrets.isEmpty()) {
        for (const auto &sec : state.default_secrets)
            m_secrets.append(qMakePair(aibox::ffi::fromRust(sec.name), aibox::ffi::fromRust(sec.value)));
    }

    m_backupArchives = aibox::ffi::fromRustJson(state.backup_archives_json).toList();
    m_lastPoolInfo = StoragePoolInfo{
        state.pool_valid,
        aibox::ffi::fromRust(state.pool_name),
        aibox::ffi::fromRust(state.pool_driver),
        state.pool_used_bytes,
        state.pool_total_bytes,
        state.pool_used_percent,
        aibox::ffi::fromRust(state.pool_size_text),
        state.pool_resizable,
    };
    m_lastPoolAllocatedBytes = state.pool_allocated_bytes;
    m_lastPoolUnboundedCount = state.pool_unbounded_count;
    m_poolInfo = aibox::ffi::fromRustJson(state.pool_info_json).toMap();
    m_recordedSessions = aibox::ffi::fromRustJson(state.recorded_sessions_json).toList();
    refreshStaleReport();
}
