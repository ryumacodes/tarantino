@compute @workgroup_size(8,8)
fn cursor_tile(@builtin(global_invocation_id) id:vec3<u32>) {
    if(any(id.xy>=vec2<u32>(256u))) {return;}
    var color=vec4<f32>(0.0);var unused=vec4<f32>(0.0);
    let tile_color=select(vec3<f32>(u.cursor_color_r,u.cursor_color_g,u.cursor_color_b),
        vec3<f32>(u.cursor_highlight_r,u.cursor_highlight_g,u.cursor_highlight_b),u.is_clicking>0.5);
    draw_cursor_procedural(vec2<f32>(id.xy)+vec2<f32>(0.5),vec2<f32>(96.0),4.0,0.0,1.0,tile_color,&color,&unused);
    textureStore(output_tex,vec2<i32>(id.xy),vec4<f32>(color.rgb*color.a,color.a));
}
