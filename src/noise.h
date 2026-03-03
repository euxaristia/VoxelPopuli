#ifndef NOISE_H
#define NOISE_H

float Perlin2D(float x, float y, float frequency, int octaves);
float Perlin3D(float x, float y, float z, float frequency, int octaves);
void InitNoise();

#endif
