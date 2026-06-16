import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
}

// NDK revision used both by AGP and by the cargo-ndk cross-compile task.
val rustNdkVersion = "29.0.14206865"

android {
    namespace = "org.ghostsinthelab.app.hypercatalog"
    compileSdk {
        version = release(37)
    }

    // NDK used to cross-compile the Rust core (see the cargoNdkBuild task below).
    ndkVersion = rustNdkVersion

    defaultConfig {
        applicationId = "org.ghostsinthelab.app.hypercatalog"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // Ship only the ABIs we cross-compile the Rust .so for.
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
}

dependencies {
    implementation(libs.androidx.activity.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.constraintlayout)
    implementation(libs.androidx.core.ktx)
    implementation(libs.material)
    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(libs.androidx.junit)
}

// --- Rust core build -------------------------------------------------------
// Cross-compiles rust/hyperffi into app/src/main/jniLibs/<abi>/libhyperffi.so using
// cargo-ndk. We invoke cargo-ndk via a plain Exec task rather than a third-party
// Rust/Gradle plugin, because AGP 9 / Gradle 9 plugin support is still uncertain.
//
// Prerequisites (documented in rust/README): rustup with the android targets installed,
// and `cargo install cargo-ndk`.
val rustDir = rootProject.layout.projectDirectory.dir("rust").asFile
val jniLibsDir = layout.projectDirectory.dir("src/main/jniLibs").asFile

fun resolveCargo(): String {
    System.getenv("CARGO")?.let { if (file(it).exists()) return it }
    val home = System.getProperty("user.home")
    val cargoHome = file("$home/.cargo/bin/cargo")
    return if (cargoHome.exists()) cargoHome.absolutePath else "cargo"
}

fun resolveSdkDir(): String {
    System.getenv("ANDROID_HOME")?.let { return it }
    System.getenv("ANDROID_SDK_ROOT")?.let { return it }
    val lp = file("${rootDir}/local.properties")
    if (lp.exists()) {
        val props = Properties()
        lp.inputStream().use { props.load(it) }
        props.getProperty("sdk.dir")?.let { return it }
    }
    return "${System.getProperty("user.home")}/Android/Sdk"
}

val cargoNdkBuild by tasks.registering(Exec::class) {
    group = "rust"
    description = "Cross-compile the Rust core (hyperffi) for Android via cargo-ndk."
    workingDir = rustDir
    // Point cargo-ndk at the NDK selected above.
    environment("ANDROID_NDK_HOME", "${resolveSdkDir()}/ndk/$rustNdkVersion")
    commandLine(
        resolveCargo(), "ndk",
        "-t", "arm64-v8a",
        "-t", "x86_64",
        "-o", jniLibsDir.absolutePath,
        "build", "--release", "-p", "hyperffi",
    )
    // Only attempt the native build when the Rust workspace is present.
    onlyIf { file("$rustDir/Cargo.toml").exists() }
}

// Ensure the .so exists before the APK is assembled.
tasks.named("preBuild") {
    dependsOn(cargoNdkBuild)
}