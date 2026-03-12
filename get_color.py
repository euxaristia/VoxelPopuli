import sys
from PIL import Image
img = Image.open(sys.argv[1]).convert('RGB')
pixels = list(img.getdata())
avg_r = sum(p[0] for p in pixels)//len(pixels)
avg_g = sum(p[1] for p in pixels)//len(pixels)
avg_b = sum(p[2] for p in pixels)//len(pixels)
print(f"Average: {avg_r}, {avg_g}, {avg_b}")
