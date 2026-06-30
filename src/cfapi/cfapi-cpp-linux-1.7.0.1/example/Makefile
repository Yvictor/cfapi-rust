sample: sample.C ../lib/libcfapi.a
#	 g++ -o sample sample.C -l pthread -L ../lib -L ../dbcapi -l:libcfapi.a -ldbcapi64 -lmd564 -lpkware64 -lport64  -std=c++11 -I ../include/ -D_GLIBCXX_USE_CXX11_ABI=1 -Wnon-virtual-dtor #use static lib for cfapi and dynamic libs for dbcapi
#	 g++ -o sample sample.C -l pthread -L ../lib -L ../dbcapi -lcfapi -ldbcapi64 -lmd564 -lpkware64 -lport64  -std=c++11 -I ../include/ -D_GLIBCXX_USE_CXX11_ABI=1 -Wnon-virtual-dtor #use dynamic lib for cfapi and dynamic libs for dbcapi
	g++ -o sample sample.C -g -l pthread -L ../lib -L ../dbcapi -l:libcfapi.a -l:libdbcapi64.a -l:libmd564.a -l:libpkware64.a -l:libport64.a  -std=c++11 -I ../include/ -D_GLIBCXX_USE_CXX11_ABI=1 -Wnon-virtual-dtor -Xlinker -Map=sample.output.map #use static lib

clean:
	-rm -f sample
