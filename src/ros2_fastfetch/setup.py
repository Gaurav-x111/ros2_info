from setuptools import setup, find_packages

setup(
    name='ros2_info',
    version='3.1.0',
    description='Fastfetch-like system info tool for ROS2 with Web UI',
    license='MIT',
    packages=find_packages(),
    python_requires='>=3.8',
    install_requires=['click', 'rich', 'psutil', 'flask', 'climage', 'Pillow', 'pyyaml'],
    entry_points={
        'console_scripts': [
            'ros2_info = fetch_info.cli:main',
        ],
    },
    package_data={
        'fetch_info': ['templates/*.html'],
    },
    include_package_data=True,
)
