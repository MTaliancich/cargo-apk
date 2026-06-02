use crate::error::NdkError;
use serde::{Deserialize, Serialize, Serializer};
use std::{fs::File, path::Path};

/// Android [manifest element](https://developer.android.com/guide/topics/manifest/manifest-element), containing an [`Application`] element.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "manifest")]
pub struct AndroidManifest {
    #[serde(rename(serialize = "@xmlns:android"))]
    #[serde(default = "default_namespace")]
    ns_android: String,
    #[serde(rename(serialize = "@package"))]
    pub package: String,
    #[serde(rename(serialize = "@android:sharedUserId"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_user_id: Option<String>,
    #[serde(rename(serialize = "@android:versionCode"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_code: Option<u32>,
    #[serde(rename(serialize = "@android:versionName"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_name: Option<String>,
    #[serde(rename(serialize = "@android:installLocation"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_location: Option<String>,
    #[serde(rename(serialize = "uses-sdk"))]
    #[serde(default)]
    pub sdk: Sdk,
    #[serde(rename(serialize = "uses-feature"))]
    #[serde(default)]
    pub uses_feature: Vec<Feature>,
    #[serde(rename(serialize = "uses-permission"))]
    #[serde(default)]
    pub uses_permission: Vec<Permission>,

    #[serde(default)]
    pub queries: Option<Queries>,

    #[serde(default)]
    pub application: Application,
}

impl Default for AndroidManifest {
    fn default() -> Self {
        Self {
            ns_android: default_namespace(),
            package: Default::default(),
            shared_user_id: Default::default(),
            version_code: Default::default(),
            version_name: Default::default(),
            install_location: Default::default(),
            sdk: Default::default(),
            uses_feature: Default::default(),
            uses_permission: Default::default(),
            queries: Default::default(),
            application: Default::default(),
        }
    }
}

impl AndroidManifest {
    pub fn write_to(&self, dir: &Path) -> Result<(), NdkError> {
        let file = File::create(dir.join("AndroidManifest.xml"))?;
        let w = std::io::BufWriter::new(file);
        quick_xml::se::to_utf8_io_writer(w, &self)?;
        Ok(())
    }
}

/// Android [application element](https://developer.android.com/guide/topics/manifest/application-element), containing an [`Activity`] element.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "application")]
pub struct Application {
    #[serde(rename(serialize = "@android:debuggable"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debuggable: Option<bool>,
    #[serde(rename(serialize = "@android:theme"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(rename(serialize = "@android:hasCode"))]
    #[serde(default)]
    pub has_code: bool,
    #[serde(rename(serialize = "@android:icon"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(rename(serialize = "@android:label"))]
    #[serde(default)]
    pub label: String,
    #[serde(rename(serialize = "@android:extractNativeLibs"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extract_native_libs: Option<bool>,
    #[serde(rename(serialize = "@android:usesCleartextTraffic"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uses_cleartext_traffic: Option<bool>,

    #[serde(rename(serialize = "meta-data"))]
    #[serde(default)]
    pub meta_data: Vec<MetaData>,
    #[serde(default)]
    pub activity: Activity,
    #[serde(rename(serialize = "receiver"))]
    #[serde(default)]
    pub receivers: Vec<Receiver>,
    #[serde(rename(serialize = "profileable"))]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profileable: Option<Profileable>,
}

/// Android [activity element](https://developer.android.com/guide/topics/manifest/activity-element).
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "activity")]
pub struct Activity {
    #[serde(rename(serialize = "@android:configChanges"))]
    #[serde(default = "default_config_changes")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_changes: Option<String>,
    #[serde(rename(serialize = "@android:label"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename(serialize = "@android:launchMode"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch_mode: Option<String>,
    #[serde(rename(serialize = "@android:name"))]
    #[serde(default = "default_activity_name")]
    pub name: String,
    #[serde(rename(serialize = "@android:screenOrientation"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    #[serde(rename(serialize = "@android:exported"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exported: Option<bool>,
    #[serde(rename(serialize = "@android:resizeableActivity"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resizeable_activity: Option<bool>,
    #[serde(rename(serialize = "@android:alwaysRetainTaskState"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always_retain_task_state: Option<bool>,

    #[serde(rename(serialize = "meta-data"))]
    #[serde(default)]
    pub meta_data: Vec<MetaData>,
    /// If no `MAIN` action exists in any intent filter, a default `MAIN` filter is serialized by `cargo-apk`.
    #[serde(rename(serialize = "intent-filter"))]
    #[serde(default)]
    pub intent_filter: Vec<IntentFilter>,
}

impl Default for Activity {
    fn default() -> Self {
        Self {
            config_changes: default_config_changes(),
            label: None,
            launch_mode: None,
            name: default_activity_name(),
            orientation: None,
            exported: None,
            resizeable_activity: None,
            always_retain_task_state: None,
            meta_data: Default::default(),
            intent_filter: Default::default(),
        }
    }
}

/// Android [receiver element](https://developer.android.com/guide/topics/manifest/receiver-element).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Receiver {
    #[serde(rename(serialize = "@android:name"))]
    pub name: String,
    #[serde(rename(serialize = "@android:enabled"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(rename(serialize = "@android:exported"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub exported: Option<bool>,
    #[serde(rename(serialize = "intent-filter"))]
    #[serde(default)]
    pub intent_filter: Vec<IntentFilter>,
}

/// Android [profileable element](https://developer.android.com/guide/topics/manifest/profileable-element).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Profileable {
    #[serde(rename(serialize = "@android:shell"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<bool>,
    #[serde(rename(serialize = "@android:enabled"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// Android [intent filter element](https://developer.android.com/guide/topics/manifest/intent-filter-element).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "intent-filter")]
pub struct IntentFilter {
    /// Serialize strings wrapped in `<action android:name="..." />`
    #[serde(serialize_with = "serialize_actions")]
    #[serde(rename(serialize = "action"))]
    #[serde(default)]
    pub actions: Vec<String>,
    /// Serialize as vector of structs for proper xml formatting
    #[serde(serialize_with = "serialize_catergories")]
    #[serde(rename(serialize = "category"))]
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(rename(serialize = "data"))]
    #[serde(default)]
    pub data: Vec<IntentFilterData>,
}

fn serialize_actions<S>(actions: &[String], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeSeq;

    #[derive(Serialize)]
    struct Action {
        #[serde(rename(serialize = "@android:name"))]
        name: String,
    }
    let mut seq = serializer.serialize_seq(Some(actions.len()))?;
    for action in actions {
        seq.serialize_element(&Action {
            name: action.clone(),
        })?;
    }
    seq.end()
}

fn serialize_catergories<S>(categories: &[String], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeSeq;

    #[derive(Serialize)]
    struct Category {
        #[serde(rename(serialize = "@android:name"))]
        pub name: String,
    }

    let mut seq = serializer.serialize_seq(Some(categories.len()))?;
    for category in categories {
        seq.serialize_element(&Category {
            name: category.clone(),
        })?;
    }
    seq.end()
}

/// Android [intent filter data element](https://developer.android.com/guide/topics/manifest/data-element).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "data")]
pub struct IntentFilterData {
    #[serde(rename(serialize = "@android:scheme"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    #[serde(rename(serialize = "@android:host"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(rename(serialize = "@android:port"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    #[serde(rename(serialize = "@android:path"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(rename(serialize = "@android:pathPattern"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_pattern: Option<String>,
    #[serde(rename(serialize = "@android:pathPrefix"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    #[serde(rename(serialize = "@android:mimeType"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Android [meta-data element](https://developer.android.com/guide/topics/manifest/meta-data-element).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "meta-data")]
pub struct MetaData {
    #[serde(rename(serialize = "@android:name"))]
    pub name: String,
    #[serde(rename(serialize = "@android:value"))]
    pub value: String,
}

/// Android [uses-feature element](https://developer.android.com/guide/topics/manifest/uses-feature-element).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "uses-feature")]
pub struct Feature {
    #[serde(rename(serialize = "@android:name"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename(serialize = "@android:required"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// The `version` field is currently used for the following features:
    ///
    /// - `name="android.hardware.vulkan.compute"`: The minimum level of compute features required. See the [Android documentation](https://developer.android.com/reference/android/content/pm/PackageManager#FEATURE_VULKAN_HARDWARE_COMPUTE)
    ///   for available levels and the respective Vulkan features required/provided.
    ///
    /// - `name="android.hardware.vulkan.level"`: The minimum Vulkan requirements. See the [Android documentation](https://developer.android.com/reference/android/content/pm/PackageManager#FEATURE_VULKAN_HARDWARE_LEVEL)
    ///   for available levels and the respective Vulkan features required/provided.
    ///
    /// - `name="android.hardware.vulkan.version"`: Represents the value of Vulkan's `VkPhysicalDeviceProperties::apiVersion`. See the [Android documentation](https://developer.android.com/reference/android/content/pm/PackageManager#FEATURE_VULKAN_HARDWARE_VERSION)
    ///   for available levels and the respective Vulkan features required/provided.
    #[serde(rename(serialize = "@android:version"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(rename(serialize = "@android:glEsVersion"))]
    #[serde(serialize_with = "serialize_opengles_version")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opengles_version: Option<(u8, u8)>,
}

fn serialize_opengles_version<S>(
    version: &Option<(u8, u8)>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match version {
        Some(version) => {
            let opengles_version = format!("0x{:04}{:04}", version.0, version.1);
            serializer.serialize_some(&opengles_version)
        }
        None => serializer.serialize_none(),
    }
}

/// Android [uses-permission element](https://developer.android.com/guide/topics/manifest/uses-permission-element).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "uses-permission")]
pub struct Permission {
    #[serde(rename(serialize = "@android:name"))]
    pub name: String,
    #[serde(rename(serialize = "@android:maxSdkVersion"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_sdk_version: Option<u32>,
}

/// Android [package element](https://developer.android.com/guide/topics/manifest/queries-element#package).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "package")]
pub struct Package {
    #[serde(rename(serialize = "@android:name"))]
    pub name: String,
}

/// Android [provider element](https://developer.android.com/guide/topics/manifest/queries-element#provider).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "provider")]
pub struct QueryProvider {
    #[serde(rename(serialize = "@android:authorities"))]
    pub authorities: String,

    // The specs say only an `authorities` attribute is required for providers contained in a `queries` element
    // however this is required for aapt support and should be made optional if/when cargo-apk migrates to aapt2
    #[serde(rename(serialize = "@android:name"))]
    pub name: String,
}

/// Android [queries element](https://developer.android.com/guide/topics/manifest/queries-element).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "queries")]
pub struct Queries {
    #[serde(rename(serialize = "package"))]
    #[serde(default)]
    pub package: Vec<Package>,
    #[serde(rename(serialize = "intent"))]
    #[serde(default)]
    pub intent: Vec<IntentFilter>,
    #[serde(rename(serialize = "provider"))]
    #[serde(default)]
    pub provider: Vec<QueryProvider>,
}

/// Android [uses-sdk element](https://developer.android.com/guide/topics/manifest/uses-sdk-element).
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "uses-sdk")]
pub struct Sdk {
    #[serde(rename(serialize = "@android:minSdkVersion"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_sdk_version: Option<u32>,
    #[serde(rename(serialize = "@android:targetSdkVersion"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_sdk_version: Option<u32>,
    #[serde(rename(serialize = "@android:maxSdkVersion"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_sdk_version: Option<u32>,
}

impl Default for Sdk {
    fn default() -> Self {
        Self {
            min_sdk_version: Some(29),
            target_sdk_version: None,
            max_sdk_version: None,
        }
    }
}

fn default_namespace() -> String {
    "http://schemas.android.com/apk/res/android".to_string()
}

fn default_activity_name() -> String {
    "android.app.NativeActivity".to_string()
}

fn default_config_changes() -> Option<String> {
    Some("orientation|keyboardHidden|screenSize".to_string())
}