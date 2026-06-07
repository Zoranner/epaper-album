param(
    [string]$InputPath = "$env:USERPROFILE\Desktop\sample.jpg",
    [string]$OutputPath = "$env:USERPROFILE\Desktop\test.bmp"
)

Add-Type -AssemblyName System.Drawing

$width = 800
$height = 480
$palette = @(
    @{ R = 0; G = 0; B = 0 },
    @{ R = 255; G = 255; B = 255 },
    @{ R = 255; G = 0; B = 0 },
    @{ R = 0; G = 255; B = 0 },
    @{ R = 0; G = 0; B = 255 },
    @{ R = 255; G = 255; B = 0 }
)

function Get-NearestPaletteEntry {
    param(
        [double]$R,
        [double]$G,
        [double]$B
    )

    $best = $palette[0]
    $bestDistance = [int64]::MaxValue

    foreach ($color in $palette) {
        $dr = $R - $color.R
        $dg = $G - $color.G
        $db = $B - $color.B
        $distance = $dr * $dr + $dg * $dg + $db * $db

        if ($distance -lt $bestDistance) {
            $bestDistance = $distance
            $best = $color
        }
    }

    return $best
}

function Limit-Channel {
    param([double]$Value)

    if ($Value -lt 0) {
        return 0.0
    }

    if ($Value -gt 255) {
        return 255.0
    }

    return $Value
}

function Add-Error {
    param(
        [double[]]$Work,
        [int]$X,
        [int]$Y,
        [double]$ErrorR,
        [double]$ErrorG,
        [double]$ErrorB,
        [double]$Weight
    )

    if ($X -lt 0 -or $X -ge $width -or $Y -lt 0 -or $Y -ge $height) {
        return
    }

    $index = ($Y * $width + $X) * 3
    $Work[$index] = Limit-Channel ($Work[$index] + $ErrorR * $Weight / 16.0)
    $Work[$index + 1] = Limit-Channel ($Work[$index + 1] + $ErrorG * $Weight / 16.0)
    $Work[$index + 2] = Limit-Channel ($Work[$index + 2] + $ErrorB * $Weight / 16.0)
}

$source = [System.Drawing.Image]::FromFile($InputPath)

try {
    $sourceAspect = $source.Width / $source.Height
    $targetAspect = $width / $height

    if ($sourceAspect -gt $targetAspect) {
        $cropHeight = $source.Height
        $cropWidth = [int]($source.Height * $targetAspect)
        $cropX = [int](($source.Width - $cropWidth) / 2)
        $cropY = 0
    } else {
        $cropWidth = $source.Width
        $cropHeight = [int]($source.Width / $targetAspect)
        $cropX = 0
        $cropY = [int](($source.Height - $cropHeight) / 2)
    }

    $bitmap = New-Object System.Drawing.Bitmap($width, $height, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    try {
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $graphics.DrawImage(
                $source,
                (New-Object System.Drawing.Rectangle(0, 0, $width, $height)),
                (New-Object System.Drawing.Rectangle($cropX, $cropY, $cropWidth, $cropHeight)),
                [System.Drawing.GraphicsUnit]::Pixel
            )
        } finally {
            $graphics.Dispose()
        }

        $work = New-Object 'double[]' ($width * $height * 3)

        for ($y = 0; $y -lt $height; $y++) {
            for ($x = 0; $x -lt $width; $x++) {
                $pixel = $bitmap.GetPixel($x, $y)
                $index = ($y * $width + $x) * 3
                $work[$index] = $pixel.R
                $work[$index + 1] = $pixel.G
                $work[$index + 2] = $pixel.B
            }
        }

        for ($y = 0; $y -lt $height; $y++) {
            for ($x = 0; $x -lt $width; $x++) {
                $index = ($y * $width + $x) * 3
                $oldR = $work[$index]
                $oldG = $work[$index + 1]
                $oldB = $work[$index + 2]
                $nearest = Get-NearestPaletteEntry -R $oldR -G $oldG -B $oldB

                $bitmap.SetPixel($x, $y, ([System.Drawing.Color]::FromArgb($nearest.R, $nearest.G, $nearest.B)))

                $errorR = $oldR - $nearest.R
                $errorG = $oldG - $nearest.G
                $errorB = $oldB - $nearest.B

                Add-Error -Work $work -X ($x + 1) -Y $y -ErrorR $errorR -ErrorG $errorG -ErrorB $errorB -Weight 7
                Add-Error -Work $work -X ($x - 1) -Y ($y + 1) -ErrorR $errorR -ErrorG $errorG -ErrorB $errorB -Weight 3
                Add-Error -Work $work -X $x -Y ($y + 1) -ErrorR $errorR -ErrorG $errorG -ErrorB $errorB -Weight 5
                Add-Error -Work $work -X ($x + 1) -Y ($y + 1) -ErrorR $errorR -ErrorG $errorG -ErrorB $errorB -Weight 1
            }
        }

        $bitmap.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Bmp)
    } finally {
        if ($bitmap) {
            $bitmap.Dispose()
        }
    }
} finally {
    $source.Dispose()
}

Write-Output "Generated $OutputPath"
