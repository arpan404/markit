pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }
dependencyResolutionManagement { repositories { google(); mavenCentral() } }
rootProject.name = "markit-android"
include(":markit")
include(":demo")
project(":demo").projectDir = file("../../examples/android")
