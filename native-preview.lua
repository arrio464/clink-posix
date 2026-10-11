-- Native Windows POSIX-style interactive subset for Clink.
-- px auto | px on | px off | px status | px cmd <raw CMD> | px <enhanced line>
local source = debug.getinfo(1, 'S').source:sub(2)
local directory = source:match('^(.*)[/\\]')
local helper = directory .. '/clink-posix-native.exe'

settings.add('posix.mode', 'auto', 'POSIX subset: auto, on, or off',
    'auto routes recognizable POSIX syntax; on routes all commands; off keeps CMD.')

local session_mode
local pending
local serial = 0
local function mode() return session_mode or settings.get('posix.mode') or 'auto' end
local function hex(value)
    return (value:gsub('.', function(c) return string.format('%02x', c:byte()) end))
end

local function unhex(value)
    if #value % 2 ~= 0 or value:find('[^%x]') then return nil end
    local text = value:gsub('%x%x', function(c) return string.char(tonumber(c,16)) end)
    if text:find('\0',1,true) then return nil end
    return text
end
local function cleanup(paths)
    if paths then for _, path in pairs(paths) do os.remove(path); os.remove(path..'.new') end end
end
local function restore_state()
    if not pending then return end
    local f = io.open(pending.state,'rb')
    if f then
        local text = f:read('*a'); f:close()
        -- Validate the complete committed snapshot before touching the parent.
        local updates, valid, cwd = {}, text:match('^V\t1\nR\t%-?%d+\n') ~= nil and text:sub(-2)=='E\n', nil
        for line in text:gmatch('[^\n]+') do
            local kind, a, b = line:match('^(%u)\t([^\t]*)\t?(.*)$')
            if kind == 'C' then cwd=unhex(a); valid=valid and cwd~=nil
            elseif kind == 'S' or kind == 'U' then
                local key, value=unhex(a), kind=='S' and unhex(b) or nil
                valid=valid and key~=nil and key:match('^[%a_][%w_]*$')~=nil and (kind=='U' or value~=nil)
                updates[#updates+1]={key=key,value=value}
            elseif kind~='V' and kind~='R' and line~='E' then valid=false end
        end
        if valid and cwd then
            for _, update in ipairs(updates) do os.setenv(update.key,update.value) end
        else print('clink-native: invalid session state; parent environment was not changed') end
    end
    cleanup(pending)
    pending=nil
end
clink.onbeginedit(restore_state)

local function enhanced(line)
    -- Deliberately conservative CMD builtins retain their established syntax.
    local first = (line:match('^%s*([^%s]+)') or ''):lower()
    local native = {clink=true, set=true, setlocal=true, endlocal=true, doskey=true,
        start=true, call=true, exit=true, rem=true, ['if']=true, ['for']=true,
        cmd=true, ['cmd.exe']=true, color=true, title=true, chcp=true}
    if native[first] then return false end
    local unix = {cat=true, sort=true, uniq=true, head=true, tail=true, wc=true,
        tr=true, cut=true, tee=true, printf=true, mkdir=true, rmdir=true,
        touch=true, cp=true, mv=true, rm=true, env=true, printenv=true,
        seq=true, basename=true, dirname=true, find=true, xargs=true,
        grep=true, sed=true, awk=true, diff=true, bash=true, sh=true}
    if unix[first] then return true end
    if first == 'export' or first == 'unset' or first == 'pwd' then return true end
    if line:match('^%s*[%a_][%w_]*=') then return true end
    if line:match('^%s*%(') or line:find('\\ ', 1, true) or line:find('\\\n', 1, true) then return true end
    if line:find("'", 1, true) or line:find('$', 1, true) or line:find('<(', 1, true)
        or line:find('<<<', 1, true) or line:find('&>', 1, true)
        or line:find('|&', 1, true) then return true end
    -- Scan unquoted characters; ordinary quoted Windows arguments remain CMD.
    local quote, i = false, 1
    while i <= #line do
        local c = line:sub(i, i)
        if c == '^' then i = i + 1
        elseif c == '"' then quote = not quote
        elseif not quote then
            if c == ';' or c == '|' or c == '*' or c == '?' or c == '{' or c == '`' then return true end
            if c == '~' and (i == 1 or line:sub(i-1,i-1):match('%s')) then return true end
            if c == '#' and (i == 1 or line:sub(i-1,i-1):match('%s')) then return true end
        end
        i = i + 1
    end
    return false
end

local function filter(line)
    restore_state()
    -- nx remains an alias for explicit native execution.
    if line:match('^%s*nx%s*$') then line='px' end
    line=line:gsub('^%s*nx%s+','px ')
    local forced = false
    if line:match('^%s*px%s*$') then
        print('clink-native: '..mode()..'; px on|auto|off, px cmd <CMD>, px <enhanced command>')
        return '', false
    end
    local tail = line:match('^%s*px%s+(.+)$')
    if tail then
        local option = tail:match('^%s*(.-)%s*$')
        if option == 'on' or option == 'off' or option == 'auto' then
            session_mode = option
            print('clink-native: '..option..' (this session)')
            return '', false
        elseif option == 'status' then
            print('clink-native: '..mode())
            return '', false
        end
        local raw = tail:match('^cmd%s+(.+)$')
        if raw then return raw, false end
        line, forced = tail, true
    end
    if not forced and (mode() == 'off' or (mode() ~= 'on' and not enhanced(line))) then return end
    local probe = io.open(helper, 'rb')
    if not probe then print('clink-native: helper missing; use px cmd to run in CMD'); return '', false end
    probe:close()
    serial = serial + 1
    local base = os.gettemppath() .. '/clink-posix-' .. clink.getsession() .. '-' .. serial
    local paths = {request=base..'.request', state=base..'.state', aliases=base..'.aliases', sync=base..'.cmd'}

    for _,path in pairs(paths) do
        if path:find('[%%!"\r\n]') then
            print('clink-native: unsafe CMD transport path'); return '',false
        end
    end
    if helper:find('[%%!"\r\n]') then print('clink-native: unsafe runtime path'); return '',false end
    local function write(path,text)
        local f=io.open(path,'wb')
        if not f then return false end
        local wrote=f:write(text); local closed=f:close()
        return wrote and closed
    end
    local aliases={}
    for _,name in ipairs(os.getaliases()) do
        local value=os.getalias(name)
        if value then aliases[#aliases+1]=hex(name)..'\t'..hex(value)..'\n' end
    end
    if not write(paths.request,line) or not write(paths.aliases,table.concat(aliases)) then
        cleanup(paths); print('clink-native: cannot write request'); return '',false
    end
    pending=paths
    return string.format('"%s" --request "%s" --state "%s" --aliases "%s" --sync "%s" --status %d & if exist "%s" call "%s"',
        helper,paths.request,paths.state,paths.aliases,paths.sync,
        tonumber(os.getenv('ERRORLEVEL')) or 0,paths.sync,paths.sync),false

end
clink.onfilterinput(filter)

if clink.argmatcher then
    clink.argmatcher('px'):addarg({'on', 'off', 'auto', 'status', 'cmd'})
end

-- Export for isolated tests without injecting into an interactive CMD session.
return {filter=filter, restore=restore_state, enhanced=enhanced}
