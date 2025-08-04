{% macro header(title, synopsis) %}
# {{ title }}

> {{ synopsis }}
{% endmacro title %}

{% macro today() %}{{ now | date("%A, %B %d, %Y") }}{% endmacro title %}
