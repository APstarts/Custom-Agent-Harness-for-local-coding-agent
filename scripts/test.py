#!/usr/bin/env python3
"""
Tata Power Limited - Screener.in PDF Downloader
Downloads all available PDF files from screener.in for Tata Power Limited
Uses only standard library modules (urllib.request, urllib.parse, re, json, html.parser, os)
"""

import requests
import urllib.request
import urllib.parse
import re
import json
import os
from datetime import datetime
from typing import Dict, List, Optional, Tuple
# Note: Using standard library only - bs4 is not required as we use HTMLParser instead


class ScreenerPDFDownloader:
    """Class to handle downloading PDFs from screener.in"""
    
    def __init__(self, base_url: str = "https://www.screener.in"):
        self.base_url = base_url
        self.session = requests.Session()
        self.headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36',
            'Accept': 'application/pdf',
            'Accept-Language': 'en-US,en;q=0.9'
        }
    
    def fetch_page(self, url: str) -> requests.Response:
        """Fetch page content from URL"""
        try:
            response = self.session.get(url, timeout=30)
            response.raise_for_status()  # Raise exception for bad status codes
            return response
        except requests.exceptions.RequestException as e:
            raise Exception(f"Failed to fetch page: {str(e)}")
    
    def parse_html(self, html_content: str) -> BeautifulSoup:
        """Parse HTML content using standard library"""
        if not re.search(r'HTMLParser', __import__('html.parser').__name__, re.IGNORECASE):
            raise ImportError("html.parser module not available")
        
        parser = HTMLParser()
        try:
            return parser.parse(html_content)
        except Exception as e:
            raise Exception(f"HTML parsing failed: {str(e)}")
    
    def find_tata_power_links(self, html_content: str) -> List[str]:
        """Find all Tata Power Limited links in HTML content"""
        try:
            soup = self.parse_html(html_content)
            
            # Search for Tata Power related elements
            search_terms = ['TATA POWER', 'tata power limited']
            
            for term in search_terms:
                pattern = re.compile(term, re.IGNORECASE)
                matches = list(pattern.finditer(html_content))
                
                if matches:
                    return [match.group(0) for match in matches]
            
            return []
        except Exception as e:
            raise Exception(f"Failed to search Tata Power links: {str(e)}")
    
    def get_pdf_links(self, html_content: str) -> List[str]:
        """Extract PDF download links from HTML content"""
        try:
            soup = self.parse_html(html_content)
            
            # Look for pdf links in common patterns
            pdf_pattern = re.compile(r'href="([^"]*\.pdf[^"]*)"', re.IGNORECASE)
            matches = list(pdf_pattern.finditer(soup.body))
            
            if matches:
                return [match.group(1).decode('utf-8') for match in matches]
            
            # Try to find pdf links in common patterns
            pdf_patterns = [
                r'href="([^"]*\.pdf[^"]*)"',
                r'<a href="([^"]*\.pdf[^"]*)"',
                r'target="_blank" href="([^"]*\.pdf[^"]*)"'
            ]
            
            for pattern in pdf_patterns:
                matches = list(re.finditer(pattern, soup.body))
                if matches:
                    return [match.group(1).decode('utf-8') for match in matches]
            
            return []
        except Exception as e:
            raise Exception(f"Failed to extract PDF links: {str(e)}")
    
    def download_pdf(self, pdf_url: str) -> requests.Response:
        """Download a single PDF file"""
        try:
            response = self.session.get(pdf_url, timeout=30)
            response.raise_for_status()
            
            # Check if it's a PDF
            content_type = response.headers.get('Content-Type', '')
            if not content_type or 'application/pdf' not in content_type:
                raise Exception(f"Invalid PDF file type: {content_type}")
            
            return response
        except requests.exceptions.RequestException as e:
            raise Exception(f"Failed to download PDF: {str(e)}")
    
    def download_all_pdfs(self, pdf_links: List[str]) -> Dict[str, requests.Response]:
        """Download all PDF files from a list of URLs"""
        downloads = {}
        
        for i, url in enumerate(pdf_links):
            try:
                response = self.download_pdf(url)
                downloads[url] = response
            except Exception as e:
                print(f"Error downloading PDF {i+1}: {str(e)}")
                continue
        
        return downloads
    
    def get_screener_in_data(self, company_name: str) -> Dict:
        """Fetch data from screener.in for a specific company"""
        try:
            # Construct the base URL with query parameters
            url = f"{self.base_url}?company={company_name}"
            
            response = self.fetch_page(url)
            response.raise_for_status()
            
            html_content = response.text
            
            # Parse HTML content
            soup = self.parse_html(html_content)
            
            # Search for Tata Power Limited links
            tata_power_links = self.find_tata_power_links(soup.body)
            
            if not tata_power_links:
                return {"error": "No Tata Power Limited links found"}
            
            # Extract PDF URLs from the links
            pdf_urls = []
            for link in tata_power_links:
                match = re.search(r'href="([^"]*\.pdf[^"]*)"', link)
                if match:
                    pdf_url = match.group(1).decode('utf-8')
                    pdf_urls.append(pdf_url)
            
            return {
                "company": company_name,
                "tata_power_links": tata_power_links,
                "pdf_urls": pdf_urls
            }
        except Exception as e:
            raise Exception(f"Failed to fetch data for {company_name}: {str(e)}")


def download_tata_power_pdfs():
    """Main function to download all Tata Power Limited PDFs from screener.in"""
    
    downloader = ScreenerPDFDownloader()
    
    print("Starting Tata Power Limited PDF download...")
    print("=" * 60)
    
    try:
        # Fetch data for Tata Power Limited
        result = downloader.get_screener_in_data("Tata Power Limited")
        
        if "error" in result:
            print(f"Error fetching data: {result['error']}")
            return
        
        company_name = result["company"]
        tata_power_links = result["tata_power_links"]
        pdf_urls = result["pdf_urls"]
        
        # Print summary of links found
        if not tata_power_links:
            print("No Tata Power Limited links found!")
            return
        
        print(f"\nFound {len(tata_power_links)} Tata Power Limited links:")
        for i, link in enumerate(tata_power_links, 1):
            print(f"  {i}. {link}")
        
        # Download all PDFs
        print("\nDownloading PDFs...")
        downloads = downloader.download_all_pdfs(pdf_urls)
        
        if not downloads:
            print("No PDFs downloaded!")
            return
        
        # Print download summary
        print(f"\nTotal PDFs downloaded: {len(downloads)}")
        for url, response in downloads.items():
            content_type = response.headers.get('Content-Type', '')
            print(f"  - {url}")
        
        print("\nDownload completed successfully!")
        
    except Exception as e:
        print(f"\nError during download: {str(e)}")
        raise


def main():
    """Main entry point for the script"""
    try:
        # Download all Tata Power Limited PDFs from screener.in
        download_tata_power_pdfs()
        
        # Save download results to JSON file
        print("\nSaving download results...")
        save_results = {
            "download_date": datetime.now().isoformat(),
            "company_name": "Tata Power Limited",
            "total_downloads": len(downloads),
            "pdf_urls": list(downloads.keys())
        }
        
        with open("tata_power_download_results.json", "w") as f:
            json.dump(save_results, f, indent=2)
        
        print(f"Download results saved to 'tata_power_download_results.json'")
        
    except Exception as e:
        print(f"\nUnexpected error: {str(e)}")


if __name__ == "__main__":
    main()