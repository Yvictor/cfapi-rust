sample-o:
	g++ -std=c++11 -I include/ -c -o sample.o sample.cpp 

sample-lib:
	g++ -shared -o libsample.so sample.o

sample: sample-o sample-lib

