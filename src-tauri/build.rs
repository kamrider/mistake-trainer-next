fn main() {
    println!("cargo:rerun-if-env-changed=MISTAKE_TRAINER_SUPABASE_URL");
    println!("cargo:rerun-if-env-changed=MISTAKE_TRAINER_SUPABASE_PUBLISHABLE_KEY");
    println!("cargo:rerun-if-env-changed=MISTAKE_TRAINER_SUPABASE_ANON_KEY");
    tauri_build::build()
}
