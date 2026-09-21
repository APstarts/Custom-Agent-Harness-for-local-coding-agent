import requests
import time
from bs4 import BeautifulSoup
import urllib.request
import re
from datetime import datetime
from typing import List, Dict, Optional

class IPhoneNewsRSSFetcher:
    def __init__(self):
        self.base_url = "https://news.google.com/search?q=iphone&hl=en"
        self.max_retries = 3
        self.retry_delay = 2  # seconds between retries

    def fetch_rss_feed(self) -> Optional[str]:
        """Fetch RSS feed URL and return it."""
        try:
            response = requests.get(self.base_url, timeout=10)
            if response.status_code == 200:
                return self.base_url
            else:
                raise Exception(f"Failed to fetch RSS feed: {response.status_code}")
        except requests.exceptions.RequestException as e:
            print(f"Error fetching RSS feed: {e}")
            return None

    def parse_rss_content(self, rss_url: str) -> List[Dict]:
        """Parse RSS XML and extract article data."""
        try:
            response = requests.get(rss_url, timeout=10)
            if response.status_code != 200:
                raise Exception(f"Failed to parse RSS content: {response.status_code}")

            soup = BeautifulSoup(response.text, 'xml')
            articles = []

            for item in soup.find_all('item'):
                title = item.get('title', '')
                link = item.get('link', '')
                pub_date = item.get('pubDate', '')
                content = item.get('content', '')

                if title and link:
                    articles.append({
                        'title': title,
                        'link': link,
                        'date': pub_date,
                        'content': content
                    })

            return articles
        except Exception as e:
            print(f"Error parsing RSS content: {e}")
            return []

    def get_latest_articles(self, rss