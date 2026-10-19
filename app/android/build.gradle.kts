allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

val newBuildDir: Directory =
    rootProject.layout.buildDirectory
        .dir("../../build")
        .get()
rootProject.layout.buildDirectory.value(newBuildDir)

subprojects {
    val newSubprojectBuildDir: Directory = newBuildDir.dir(project.name)
    project.layout.buildDirectory.value(newSubprojectBuildDir)
}
subprojects {
    project.evaluationDependsOn(":app")
}

// flutter_webrtc 0.12 pins compileSdk 31. Its AndroidX dependencies need 34.
// evaluationDependsOn(":app") above can finish a project before a later
// afterEvaluate is registered, so already-evaluated projects are updated now.
fun Project.raiseCompileSdk() {
    val android = extensions.findByName("android") ?: return
    val current = android.javaClass.methods
        .firstOrNull { it.name == "getCompileSdk" && it.parameterCount == 0 }
        ?.invoke(android) as? Int
    if (current != null && current >= 34) return
    val setter = android.javaClass.methods.firstOrNull { method ->
        method.name == "setCompileSdk" && method.parameterCount == 1
    } ?: android.javaClass.methods.firstOrNull { method ->
        method.name == "setCompileSdkVersion" && method.parameterCount == 1
    }
    setter?.invoke(android, 34)
}

subprojects {
    if (state.executed) {
        raiseCompileSdk()
    } else {
        afterEvaluate { raiseCompileSdk() }
    }
}

tasks.register<Delete>("clean") {
    delete(rootProject.layout.buildDirectory)
}
