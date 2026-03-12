import sys
from PIL import Image
img = Image.open(sys.argv[1]).convert('RGB')
# Resize to 16x16 just in case the provided image is larger, but it's probably already 16x16.
if img.size != (16, 16):
    img = img.resize((16, 16), Image.NEAREST)
pixels = list(img.getdata())

out = "[\n"
for y in range(16):
    out += "    "
    for x in range(16):
        p = pixels[y * 16 + x]
        out += f"({p[0]}, {p[1]}, {p[2]}), "
    out += "\n"
out += "]"
with open("sand_pixels.txt", "w") as f:
    f.write(out)
print("Done writing sand_pixels.txt")
