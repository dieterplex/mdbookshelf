# {{title}}

Last updated: {{timestamp | date(format="%Y-%m-%d %H:%M")}}

{% for entry in entries %}
{{loop.index}}. {{entry.title}} - \[EPUB\]({{entry.path | urlencode}}) ({{entry.epub_size | filesizeformat}}) | [Website](%7B%7Bentry.url%7D%7D) | [Repository](%7B%7Bentry.repo_url%7D%7D)
Commit: {{entry.commit_sha}} ({{entry.last_modified | date(format="%Y-%m-%d %H:%M")}})
{% endfor %}
