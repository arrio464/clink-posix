-- Compatibility entry point; current checks include persistent parent state.
local source=debug.getinfo(1,'S').source:sub(2):gsub('\\','/')
local directory=assert(source:match('^(.*)/[^/]+$'))
dofile(directory..'/clink_parity.lua')
