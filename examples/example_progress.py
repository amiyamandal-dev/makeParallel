#!/usr/bin/env python3
"""
Simple example demonstrating the report_progress bug fix.

This shows how easy it is now to report progress from within
a @parallel decorated function.
"""

import time
import makeparallel as mp


@mp.parallel
def download_file(filename, size_mb):
    """Simulate downloading a file with progress reporting."""
    print(f"Starting download: {filename}")

    chunks = 20
    for i in range(chunks):
        time.sleep(0.05)  # Simulate downloading a chunk
        progress = (i + 1) / chunks

        # Report progress - automatically uses thread-local task_id!
        mp.report_progress(progress)

    print(f"Completed download: {filename}")
    return f"{filename} ({size_mb}MB) downloaded"


def main():
    print("Starting file downloads with progress tracking...\n")

    # Start multiple downloads in parallel
    downloads = [
        download_file("video.mp4", 100),
        download_file("document.pdf", 5),
        download_file("image.jpg", 2),
    ]

    # Monitor progress
    print("\nMonitoring download progress:")
    print("-" * 60)

    all_done = False
    while not all_done:
        all_done = True

        for i, handle in enumerate(downloads):
            if not handle.is_ready():
                all_done = False

            progress = handle.get_progress()
            name = handle.get_name()

            # Progress bar
            filled = int(progress * 30)
            bar = "█" * filled + "░" * (30 - filled)
            print(f"{name:20s} [{bar}] {progress*100:5.1f}%")

        if not all_done:
            print("\033[F" * len(downloads), end="")  # Move cursor up
            time.sleep(0.1)

    print("\n" + "-" * 60)

    # Get results
    results = [h.get() for h in downloads]

    print("\nAll downloads completed!")
    for result in results:
        print(f"  ✓ {result}")


if __name__ == "__main__":
    main()
