use super::model::{Model, Layer};
use std::fmt::Write;

fn floats(values: &[f32]) -> String {
    values.iter().map(|v| format!("{v:.9e}")).collect::<Vec<_>>().join(",")
}
fn activate(value: &str, activation: i32) -> String {
    if activation == 0 { value.to_owned() }
    else { format!("max({}{value},vec4<f32>(0))", if activation < 0 { "-" } else { "" }) }
}
fn readout(code: &mut String, layer: &Layer, input: usize) {
    for tap in layer.taps.iter().filter(|tap| tap.input == input) {
        writeln!(code, "result{}+=mat4x4<f32>({})*{};", tap.output,
            floats(&tap.matrix), activate("feature", tap.activation)).unwrap();
    }
}

pub(super) fn generate(model: &Model) -> String {
    let mut code = include_str!("common.wgsl").to_owned();
    let groups = model.scale * model.scale / 4;
    writeln!(code, "var<workgroup> a:array<vec4<f32>,484>;
        var<workgroup> b:array<vec4<f32>,400>;
        var<workgroup> low:array<atomic<u32>,3>;
        var<workgroup> high:array<atomic<u32>,3>;
        var<workgroup> flat:u32;
        var<workgroup> flat_result:array<vec4<f32>,{groups}>;
        @compute @workgroup_size(8,8) fn tile(@builtin(workgroup_id) group:vec3<u32>,
            @builtin(local_invocation_index) thread:u32) {{
        let local=vec2<u32>(thread%8u,thread/8u);let origin=group.xy*8u;
        for(var n=thread;n<484u;n+=64u){{
            a[n]=original(vec2<i32>(origin)+vec2<i32>(i32(n%22u),i32(n/22u))-vec2<i32>(7));
        }}
        workgroupBarrier();").unwrap();
    let output = &model.layers[7];
    for (group, bias) in output.bias.chunks_exact(4).enumerate() {
        writeln!(code, "var result{group}=vec4<f32>({});", floats(bias)).unwrap();
    }
    code.push_str("if(thread==0u){for(var c=0u;c<3u;c++){atomicStore(&low[c],255u);atomicStore(&high[c],0u);}}
        workgroupBarrier();
        for(var n=thread;n<484u;n+=64u){let color=vec3<u32>(round(a[n].rgb*255.0));
            for(var c=0u;c<3u;c++){atomicMin(&low[c],color[c]);atomicMax(&high[c],color[c]);}}
        workgroupBarrier();
        if(thread==0u){var span=0u;for(var c=0u;c<3u;c++){span=max(span,atomicLoad(&high[c])-atomicLoad(&low[c]));}
            flat=select(0u,1u,span<=8u);}
        let use_flat=workgroupUniformLoad(&flat)==1u;
        if(use_flat){if(thread==0u){var feature=original(vec2<i32>(origin)+vec2<i32>(4));\n");
    for (index, layer) in model.layers[..7].iter().enumerate() {
        writeln!(code, "{{var sum=vec4<f32>({});", floats(&layer.bias)).unwrap();
        let mut matrices: Vec<(i32, [f64; 16])> = Vec::new();
        for tap in &layer.taps {
            let slot = match matrices.iter().position(|(activation, _)| *activation == tap.activation) {
                Some(slot) => slot,
                None => { matrices.push((tap.activation, [0.0; 16])); matrices.len() - 1 }
            };
            for (sum, value) in matrices[slot].1.iter_mut().zip(tap.matrix) { *sum += value as f64; }
        }
        for (activation, matrix) in matrices {
            writeln!(code, "sum+=mat4x4<f32>({})*{};", floats(&matrix.map(|v| v as f32)),
                activate("feature", activation)).unwrap();
        }
        code.push_str("feature=sum;}\n");
        readout(&mut code, output, index);
    }
    for group in 0..groups { writeln!(code, "flat_result[{group}]=result{group};").unwrap(); }
    code.push_str("}workgroupBarrier();\n");
    for group in 0..groups { writeln!(code, "result{group}=flat_result[{group}];").unwrap(); }
    code.push_str("}else{\n");
    for (index, layer) in model.layers[..7].iter().enumerate() {
        let before = 22 - index * 2;
        let after = before - 2;
        let (from, to) = if index % 2 == 0 { ("a", "b") } else { ("b", "a") };
        writeln!(code, "{{for(var n=thread;n<{}u;n+=64u){{
            let q=vec2<i32>(i32(n%{after}u),i32(n/{after}u))+vec2<i32>(1);
            var sum=vec4<f32>({});", after * after, floats(&layer.bias)).unwrap();
        for (i, tap) in layer.taps.iter().enumerate() {
            let value = format!("{from}[u32(q.y+{})*{before}u+u32(q.x+{})]", tap.offset[1], tap.offset[0]);
            writeln!(code, "let v{i}={};sum+=mat4x4<f32>({})*v{i};",
                activate(&value, tap.activation), floats(&tap.matrix)).unwrap();
        }
        writeln!(code, "{to}[n]=sum;}}workgroupBarrier();
            let feature={to}[(local.y+{}u)*{after}u+local.x+{}u];", (after - 8) / 2, (after - 8) / 2).unwrap();
        readout(&mut code, output, index);
        code.push_str("}\n");
    }
    code.push_str("}let p=origin+local;if(all(p<region.zw)){\n");
    for channel in 0..model.scale * model.scale {
        writeln!(code, "write_output(p*{}u+vec2<u32>({},{}),result{}[{}],{}u);",
            model.scale, channel % model.scale, channel / model.scale, channel / 4, channel % 4, model.scale).unwrap();
    }
    code.push_str("}}\n");
    code
}
