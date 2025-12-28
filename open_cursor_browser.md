# Opening Cursor's Simple Browser

## Method 1: Command Palette (Recommended)

1. **Press**: `Ctrl+Shift+P` (Windows/Linux) or `Cmd+Shift+P` (Mac)
2. **Type**: `Simple Browser: Show` or just `Simple Browser`
3. **Press**: Enter
4. **Enter URL**: `http://localhost:8080`
5. **Press**: Enter

## Method 2: Keyboard Shortcut

Some Cursor versions support:
- `Ctrl+Shift+B` or `Cmd+Shift+B` to open Simple Browser

## Method 3: Via Settings

1. Go to Settings (Ctrl+,)
2. Search for "Simple Browser"
3. Configure or open from there

## What You'll See

Once the browser opens and loads http://localhost:8080:

1. **Pipeline Status Section** - Shows X/4 nodes online
2. **Query Input Field** - At the top of the page
3. **Response Area** - Where results will appear
4. **Node Status** - Shows which shards are available

## Testing Inference

1. Wait for "4/4 nodes online" (may take 30-60 seconds)
2. Type in query field: `what do a cat and a snake have in common`
3. Click "Send" or press Enter
4. Watch for response below

## If Browser Doesn't Open

The web server may still be compiling. Check:
- Web server terminal window for compilation progress
- Wait for "Web Console: http://localhost:8080" message
- Then try opening the browser again

