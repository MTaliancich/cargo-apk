use android_activity::AndroidApp;
use jni::jni_str;
use jni::objects::JString;
use jni::signature::{RuntimeFieldSignature, RuntimeMethodSignature};

#[unsafe(no_mangle)]
fn android_main(_app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    enumerate_audio_devices().unwrap();
}

const GET_DEVICES_OUTPUTS: jni::sys::jint = 2;

fn enumerate_audio_devices() -> Result<(), Box<dyn std::error::Error>> {
    // Create a VM for executing Java calls
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) };

    // Premake JNI signatures
    let string_sig = RuntimeFieldSignature::from_str("Ljava/lang/String;")?;
    let string_ret_obj_sig = RuntimeMethodSignature::from_str("(Ljava/lang/String;)Ljava/lang/Object;")?;
    let int_ret_and_obj_sig = RuntimeMethodSignature::from_str("(I)[Landroid/media/AudioDeviceInfo;")?;
    let return_int_sig = RuntimeMethodSignature::from_str("()I")?;
    let return_int_array_sig = RuntimeMethodSignature::from_str("()[I")?;
    let return_char_sequence_sig = RuntimeMethodSignature::from_str("()Ljava/lang/CharSequence;")?;
    let return_string_sig = RuntimeMethodSignature::from_str("()Ljava/lang/String;")?;

    vm.attach_current_thread(|env| {
        let context = unsafe { jni::objects::JObject::from_raw(env, ctx.context().cast()) };
        let class_ctxt = env.find_class(jni_str!("android/content/Context"))?;
        let audio_service = env.get_static_field(class_ctxt, jni_str!("AUDIO_SERVICE"), &string_sig.field_signature())?;

        // Query the global Audio Service
        let audio_manager = env
            .call_method(
                context,
                jni_str!("getSystemService"),
                // JNI type signature needs to be derived from the Java API
                // (ArgTys)ResultTy
                &string_ret_obj_sig.method_signature(),
                &[(&audio_service).into()],
            )?
            .l()?;

        // Enumerate output devices
        let devices = env.call_method(
            audio_manager,
            jni_str!("getDevices"),
            &int_ret_and_obj_sig.method_signature(),
            &[GET_DEVICES_OUTPUTS.into()],
        )?;

        println!("-- Output Audio Devices --");

        let device_array = devices.l()?;
        let device_array_cast = env.as_cast::<jni::objects::JObjectArray>(&device_array)?;
        let device_array: &jni::objects::JObjectArray = device_array_cast.as_ref();
        let len = device_array.len(env)?;
        for i in 0..len {
            let device = device_array.get_element(env, i)?;

            // Collect device information
            // See https://developer.android.com/reference/android/media/AudioDeviceInfo
            let product_name: String = {
                let name =
                    env.call_method(&device, jni_str!("getProductName"), &return_char_sequence_sig.method_signature(), &[])?;
                let name = env.call_method(name.l()?, jni_str!("toString"), &return_string_sig.method_signature(), &[])?;
                let j_string = name.l()?;
                let j_string_cast = env.as_cast::<JString>(&j_string)?;

                j_string_cast.to_string()
            };
            let id = env.call_method(&device, jni_str!("getId"), &return_int_sig.method_signature(), &[])?.i()?;
            let ty = env.call_method(&device, jni_str!("getType"), &return_int_sig.method_signature(), &[])?.i()?;

            let sample_rates = {
                let sample_array = env
                    .call_method(&device, jni_str!("getSampleRates"), &return_int_array_sig.method_signature(), &[])?
                    .l()?;
                let sample_array_cast = env.as_cast::<jni::objects::JPrimitiveArray<jni::sys::jint>>(&sample_array)?;
                let sample_array: &jni::objects::JPrimitiveArray<jni::sys::jint> = sample_array_cast.as_ref();
                let len = sample_array.len(env)?;

                let mut sample_rates = vec![0; len as usize];
                sample_array.get_region(env, 0, &mut sample_rates)?;
                sample_rates
            };

            println!("Device {product_name}: Id {id}, Type {ty}");
            println!("sample rates: {sample_rates:#?}");
        }
        Ok::<(), anyhow::Error>(())
    })?;



    Ok(())
}
