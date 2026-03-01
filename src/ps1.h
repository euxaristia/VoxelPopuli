#ifndef PS1_H
#define PS1_H

#if defined(PS1_RENDERER)

static const char* ps1_vs = 
    "#version 330\n"
    "in vec3 vertexPosition;\n"
    "in vec2 vertexTexCoord;\n"
    "in vec4 vertexColor;\n"
    "uniform mat4 mvp;\n"
    "uniform float precision = 240.0;\n"
    "out vec4 fragColor;\n"
    "noperspective out vec2 fragTexCoord;\n"
    "void main() {\n"
    "    fragColor = vertexColor;\n"
    "    fragTexCoord = vertexTexCoord;\n"
    "    vec4 pos = mvp * vec4(vertexPosition, 1.0);\n"
    "    if (pos.w != 0.0) {\n"
    "        pos.xyz /= pos.w;\n"
    "        pos.xy = floor(pos.xy * precision + 0.5) / precision;\n"
    "        pos.xyz *= pos.w;\n"
    "    }\n"
    "    gl_Position = pos;\n"
    "}\n";

static const char* ps1_fs = 
    "#version 330\n"
    "in vec4 fragColor;\n"
    "noperspective in vec2 fragTexCoord;\n"
    "uniform sampler2D texture0;\n"
    "uniform vec4 colDiffuse;\n"
    "out vec4 finalColor;\n"
    "void main() {\n"
    "    vec4 texelColor = texture(texture0, fragTexCoord);\n"
    "    if (texelColor.a < 0.1) discard;\n"
    "    // PS1 color depth (5 bits per channel)\n"
    "    vec4 c = texelColor * colDiffuse * fragColor;\n"
    "    c.rgb = floor(c.rgb * 31.0 + 0.5) / 31.0;\n"
    "    finalColor = c;\n"
    "}\n";

#endif

#endif // PS1_H
