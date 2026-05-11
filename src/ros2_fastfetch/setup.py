from setuptools import setup, find_packages

setup(
    name='ros2_info',
    version='1.0.0',
    packages=find_packages(),
    install_requires=['click', 'rich', 'psutil', 'flask', 'climage', 'Pillow'],
    entry_points={
        'console_scripts': [
            'ros2_info = fetch_info.cli:main',
        ],
    },
    package_data={
        'fetch_info': ['templates/*.html', 'display/assets/*.png'],
    },
    include_package_data=True,
)
