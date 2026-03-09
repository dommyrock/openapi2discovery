# Usage py script


```python
python openapi_to_discovery.py openapi.json -o discovery.json
# or pipe
cat openapi.json | python openapi_to_discovery.py - | jq .

```