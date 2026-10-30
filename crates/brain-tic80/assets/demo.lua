-- title: Reflex Orbit
-- author: sevenc-nanashi
-- script: lua
x=120
y=68
t=0
function TIC()
 t=t+1
 if btn(0) then y=y-2 end
 if btn(1) then y=y+2 end
 if btn(2) then x=x-2 end
 if btn(3) then x=x+2 end
 local mx,my,down=mouse()
 if down then x=mx y=my end
 x=math.max(8,math.min(232,x))
 y=math.max(24,math.min(112,y))
 if btnp(4) then sfx(0,48,12,0,8) end
 cls(0)
 for i=0,11 do
  local a=t/40+i*math.pi/6
  circ(x+math.cos(a)*24,y+math.sin(a)*24,3,5+i%7)
 end
 circ(x,y,6,btn(4) and 4 or 12)
 print('TIC-80 / REFLEX',75,8,12)
 print('ARROWS MOVE   Z SOUND   TOUCH MOVE',20,124,13)
end
