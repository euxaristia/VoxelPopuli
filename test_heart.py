def draw():
    for py in range(9):
        row = ""
        for px in range(9):
            fill = False
            # top humps
            if (1 <= px <= 3 and py == 0) or (5 <= px <= 7 and py == 0):
                fill = True
            # main body
            elif 0 <= px <= 8 and 1 <= py <= 3:
                fill = True
            elif 1 <= px <= 7 and py == 4:
                fill = True
            elif 2 <= px <= 6 and py == 5:
                fill = True
            elif 3 <= px <= 5 and py == 6:
                fill = True
            elif px == 4 and py == 7:
                fill = True

            is_border = False
            # Top edge
            if (1 <= px <= 3 and py == 0) or (5 <= px <= 7 and py == 0):
                is_border = True
            
            # Inner dip
            if px == 4 and py == 1:
                is_border = True

            # Outer sides
            if (px == 0 and 1 <= py <= 3) or (px == 8 and 1 <= py <= 3):
                is_border = True
                
            # Bottom diagonal edges
            if (px == 1 and py == 4) or (px == 7 and py == 4):
                is_border = True
            if (px == 2 and py == 5) or (px == 6 and py == 5):
                is_border = True
            if (px == 3 and py == 6) or (px == 5 and py == 6):
                is_border = True
            if px == 4 and py == 7:
                is_border = True

            # Inner shading (makes it look 3D and not flat)
            # Add a white highlight to top-left
            is_highlight = False
            if px == 1 and py == 1:
                is_highlight = True
            if px == 2 and py == 1:
                is_highlight = True
            if px == 1 and py == 2:
                is_highlight = True

            if not fill:
                row += ". "
            elif is_border:
                row += "B "
            elif is_highlight:
                row += "W "
            else:
                row += "R "
        print(row)

draw()
