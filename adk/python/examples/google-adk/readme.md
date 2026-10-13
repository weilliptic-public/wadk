## run the flights a2a server
```
uvicorn flights.agent:a2a_app --host 127.0.0.1 --port 8001
```


## run the hotels a2a server
```
uvicorn hotels.agent:a2a_app --host 127.0.0.1 --port 8002
```


## run the planner
```
adk web planner/
```

then ask:
```
I want to travel from Delhi to Bombay
from October 20 to October 23.

Plan the trip for me.
```
