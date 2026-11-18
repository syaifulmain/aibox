#pragma once

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

void aibox_test_init_gui_app(void);
void aibox_test_shutdown_gui_app(void);
void aibox_test_process_events(int max_ms);

void *aibox_test_backend_new(void);
void aibox_test_backend_free(void *ptr);
void aibox_test_backend_configure(void *ptr, const char *path, bool project_mode);
int aibox_test_backend_count(void *ptr);
int aibox_test_backend_primary_action_with_spy(void *ptr);
int aibox_test_backend_enable_file_access(void *ptr, const char *name, char *buf, int buf_len);
void aibox_test_backend_disable_file_access(void *ptr, const char *name);
void aibox_test_backend_open_file_access(void *ptr, const char *name);
bool aibox_test_backend_busy(void *ptr);
bool aibox_test_backend_selected_env_empty(void *ptr);
int aibox_test_backend_env_count(void *ptr);
void aibox_test_backend_enter_environment(void *ptr, const char *name);

void *aibox_test_theme_new(void);
void aibox_test_theme_free(void *ptr);
void aibox_test_theme_configure(void *ptr, const char *dir);
bool aibox_test_theme_is_dark(void *ptr);
int aibox_test_theme_string_prop(void *ptr, const char *prop_name, char *buf, int buf_len);
void *aibox_test_theme_spy_new(void *ptr);
int aibox_test_theme_spy_count(void *spy);
void aibox_test_theme_spy_free(void *spy);

int aibox_test_qml_smoke(char *err_buf, int err_buf_len);
int aibox_test_qml_smoke_with_theme(const char *theme_dir, char *err_buf, int err_buf_len);

#ifdef __cplusplus
}
#endif
