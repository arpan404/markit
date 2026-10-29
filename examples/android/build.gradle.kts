plugins { id("com.android.application"); id("org.jetbrains.kotlin.android") }
android {
    namespace = "dev.markit.demo"
    compileSdk = 36
    sourceSets["main"].assets.srcDir("../../benchmarks/mobile")
    buildTypes { getByName("release") { signingConfig = signingConfigs.getByName("debug") } }
    defaultConfig { applicationId = "dev.markit.demo"; minSdk = 23; targetSdk = 36; versionCode = 1; versionName = "0.1.0" }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    kotlinOptions { jvmTarget = "17" }
}
dependencies { implementation(project(":markit")); implementation("androidx.recyclerview:recyclerview:1.4.0") }
