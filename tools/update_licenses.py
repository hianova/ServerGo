#!/usr/bin/env python3
import os
import subprocess
import sys

def main():
    print("Updating ThirdPartyLicenses.txt...")
    try:
        # Check if cargo-license is installed
        result = subprocess.run(['cargo', 'license', '--version'], capture_output=True)
        if result.returncode != 0:
            print("cargo-license not found. Installing cargo-license...")
            subprocess.run(['cargo', 'install', 'cargo-license'], check=True)
        
        # Generate the license file
        with open("ThirdPartyLicenses.txt", "w") as f:
            subprocess.run(['cargo', 'license'], stdout=f, check=True)
        print("ThirdPartyLicenses.txt successfully updated.")
    except Exception as e:
        print(f"Failed to update licenses: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
