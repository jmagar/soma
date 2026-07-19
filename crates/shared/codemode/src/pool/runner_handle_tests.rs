use super::runner_handle::RunnerSpawn;
use serial_test::serial;

#[test]
#[serial(code_mode_runner_exe_env)]
fn runner_spawn_current_exe_has_program() {
    assert!(RunnerSpawn::current_exe().unwrap().program.is_absolute());
}
