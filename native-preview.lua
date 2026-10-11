-- Experimental adapter, for an ISOLATED Clink --scripts session only.
-- nx <line> opts in explicitly. No automatic routing or parent-state mutation.
local file = debug.getinfo(1, 'S').source:sub(2)
local directory = assert(file:match('^(.*)[/\\]'))
local executable = directory .. '/clink-posix-native.exe'
local pending, serial = nil, 0
local function cleanup()
    if pending then os.remove(pending); pending = nil end
end
clink.onbeginedit(cleanup)
local function cmdpath(path)
    -- The prototype refuses CMD expansion characters in transport paths rather
    -- than pretending that ordinary double quotes suppress % and ! expansion.
    if path:find('[%%!"\r\n]') then return nil end
    return '"' .. path .. '"'
end
clink.onfilterinput(function(line)
    local command = line:match('^%s*nx%s+(.+)$')
    if not command then return end
    cleanup()
    serial = serial + 1
    local request = os.gettemppath() .. '/clink-native-request-' .. clink.getsession() .. '-' .. serial
    local exe_arg, request_arg = cmdpath(executable), cmdpath(request)
    if not exe_arg or not request_arg then
        print('clink-native: preview transport path contains unsupported CMD expansion characters')
        return '', false
    end
    local stream, error = io.open(request, 'wb')
    if not stream then print('clink-native: ' .. tostring(error)); return '', false end
    local wrote, write_error = stream:write(command)
    local closed, close_error = stream:close()
    if not wrote or not closed then
        os.remove(request)
        print('clink-native: ' .. tostring(write_error or close_error))
        return '', false
    end
    pending = request
    return exe_arg .. ' --request ' .. request_arg .. ' --status ' .. tostring(tonumber(os.getenv('ERRORLEVEL')) or 0), false
end)
