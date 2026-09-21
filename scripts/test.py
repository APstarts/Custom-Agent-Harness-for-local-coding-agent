#!/usr/bin/env python3
"""
Tata Power Limited - Screener.in PDF Downloader
Downloads all available PDF files from Tata Power Limited page on screener.in
"""

import requests
from bs4 import BeautifulSoup
import os
import re
import json
from datetime import datetime
from pathlib import Path
from urllib.parse import urljoin, urlparse
import logging

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


def get_screener_in_api_url(company_name):
    """
    Construct the Screener.in API URL for a given company name.
    
    Args:
        company_name (str): Name of the company to search
        
    Returns:
        str: The base API URL
    """
    # Example base URLs based on company names
    company_base_urls = {
        "Tata Power Limited": "https://screener.in/api/search",
        "Tata Motors": "https://screener.in/api/search",
        "Reliance Industries": "https://screener.in/api/search"
    }
    
    # Default to Tata Power Limited
    if company_name == "Tata Power Limited":
        return "https://screener.in/api/search"
    
    return company_base_urls.get(company_name, "https://screener.in/api/search")


def get_pdf_files_from_screener_in(company_name):
    """
    Fetch PDF files from Tata Power Limited page on screener.in
    
    Args:
        company_name (str): Name of the company to search
        
    Returns:
        list: List of PDF file URLs
    """
    try:
        # Step 1: Navigate to Tata Power Limited page on screener.in
        logger.info(f"Navigating to Tata Power Limited page...")
        
        # Construct the API URL for Tata Power Limited
        api_url = get_screener_in_api_url("Tata Power Limited")
        
        # Make HTTP request to fetch data
        response = requests.get(api_url, timeout=10)
        
        if response.status_code != 200:
            logger.error(f"Failed to fetch data from API. Status code: {response.status_code}")
            return []
        
        # Parse the HTML content using BeautifulSoup
        html_content = response.text
        
        soup = BeautifulSoup(html_content, 'html.parser')
        
        # Find all PDF links in the page
        pdf_links = []
        
        # Look for PDF links with specific patterns
        for link in soup.find_all('a', href=True):
            href = link['href']
            
            # Check if it's a PDF file
            if re.search(r'\.pdf$', href.split('?')[0]):
                pdf_links.append(href)
        
        logger.info(f"Found {len(pdf_links)} PDF files")
        
        return pdf_links
        
    except requests.exceptions.RequestException as e:
        logger.error(f"Request failed: {e}")
        return []
    except Exception as e:
        logger.error(f"Error parsing HTML: {e}")
        return []


def download_pdf_file(pdf_url):
    """
    Download a single PDF file from the given URL.
    
    Args:
        pdf_url (str): URL of the PDF file
        
    Returns:
        str: Path to the downloaded file or None if failed
    """
    try:
        # Construct the full URL
        response = requests.get(pdf_url, timeout=10)
        
        if response.status_code == 200:
            # Save the PDF file locally
            filename = os.path.basename(pdf_url)
            output_path = f"./downloads/{filename}"
            
            with open(output_path, 'wb') as f:
                f.write(response.content)
            
            logger.info(f"Successfully downloaded {filename}")
            return output_path
        
        else:
            logger.error(f"Failed to download PDF. Status code: {response.status_code}")
            return None
            
    except requests.exceptions.RequestException as e:
        logger.error(f"Download failed: {e}")
        return None


def get_company_info(company_name):
    """
    Get basic information about a company from Screener.in API.
    
    Args:
        company_name (str): Name of the company
        
    Returns:
        dict: Company information or None if not found
    """
    try:
        api_url = get_screener_in_api_url(company_name)
        
        response = requests.get(api_url, timeout=10)
        
        if response.status_code == 200:
            html_content = response.text
            
            soup = BeautifulSoup(html_content, 'html.parser')
            
            # Extract company name from page title
            company_title = soup.find('title', string=lambda x: x in ['Tata Power Limited'])
            
            if company_title:
                return {
                    'company_name': company_title.string.strip(),
                    'page_url': response.url,
                    'status': 200
                }
            else:
                return None
                
        else:
            logger.error(f"Failed to fetch company info. Status code: {response.status_code}")
            return None
            
    except requests.exceptions.RequestException as e:
        logger.error(f"Request failed: {e}")
        return None


def main():
    """
    Main function to download PDF files from Tata Power Limited on screener.in
    
    Returns:
        dict: Summary of downloaded files
    """
    # Define the company name
    company_name = "Tata Power Limited"
    
    logger.info("=" * 60)
    logger.info("Starting Tata Power Limited PDF Download Process")
    logger.info("=" * 60)
    
    # Step 1: Navigate to Tata Power Limited page on screener.in
    logger.info(f"Navigating to Tata Power Limited page...")
    
    # Construct the API URL for Tata Power Limited
    api_url = get_screener_in_api_url(company_name)
    
    # Make HTTP request to fetch data
    response = requests.get(api_url, timeout=10)
    
    if response.status_code != 200:
        logger.error(f"Failed to fetch data from API. Status code: {response.status_code}")
        return []
    
    # Parse the HTML content using BeautifulSoup
    html_content = response.text
    
    soup = BeautifulSoup(html_content, 'html.parser')
    
    # Find all PDF links in the page
    pdf_links = []
    
    # Look for PDF links with specific patterns
    for link in soup.find_all('a', href=True):
        href = link['href']
        
        # Check if it's a PDF file
        if re.search(r'\.pdf$', href.split('?')[0]):
            pdf_links.append(href)
    
    logger.info(f"Found {len(pdf_links)} PDF files")
    
    # Step 2: Download each PDF file
    downloaded_files = []
    
    for i, pdf_url in enumerate(pdf_links):
        logger.info(f"\nProcessing PDF file {i + 1}/{len(pdf_links)}...")
        
        output_path = download_pdf_file(pdf_url)
        
        if output_path:
            downloaded_files.append(output_path)
    
    # Step 3: Save the results
    result = {
        'company_name': company_name,
        'total_downloaded': len(downloaded_files),
        'downloaded_files': downloaded_files,
        'timestamp': datetime.now().isoformat()
    }
    
    logger.info("=" * 60)
    logger.info("Download Process Completed Successfully")
    logger.info(f"Total PDF files downloaded: {len(downloaded_files)}")
    logger.info("=" * 60)
    
    return result


if __name__ == "__main__":
    # Run the main function
    result = get_pdf_files_from_screener_in("Tata Power Limited")
    
    if not result:
        print("No PDF files found for Tata Power Limited on screener.in")
    else:
        print(f"Found {len(result)} PDF files for Tata Power Limited")
        
        # Save the results to a file
        with open('tata_power_limited_downloads.json', 'w') as f:
            json.dump(result, f, indent=4)
        
        print("Results saved to tata_power_limited_downloads.json")